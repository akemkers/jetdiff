//! Endringer og filinnhold fra git. Kaller `git` så resultatet blir det samme som i
//! `git diff`, med brukerens config og rename-deteksjon.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;

use jetdiff_changes::{FileChange, Status};

/// Commit-lista viser aldri flere, så en gammel default-branch ikke gir tusenvis.
const MAX_COMMITS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    /// Første linje av meldingen.
    pub subject: String,
    /// Sekunder siden 1970.
    pub time: i64,
}

/// En lenke som `https://github.com/eier/repo/pull/12`.
#[derive(Debug, PartialEq, Eq)]
pub struct PullRequest {
    pub url: String,
    /// `eier/repo`.
    pub slug: String,
    pub number: u32,
}

impl PullRequest {
    /// Godtar også lenker til en fane eller kommentar i PR-en.
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.strip_prefix("https://github.com/")?;
        let mut parts = rest.split(['/', '#', '?']);
        let (owner, repo) = (parts.next()?, parts.next()?);
        if owner.is_empty() || repo.is_empty() || parts.next()? != "pull" {
            return None;
        }
        Some(Self {
            url: url.to_owned(),
            slug: format!("{owner}/{repo}"),
            number: parts.next()?.parse().ok()?,
        })
    }
}

pub struct Repo {
    root: PathBuf,
    /// Venstre side. Det tomme treet hvis det ikke finnes noen forelder.
    left: String,
    /// `None` er working tree.
    right: Option<String>,
    /// Revisjonene commit-lista viser. `None` for en commit og for et repo uten commits.
    history: Option<String>,
}

impl Repo {
    /// En commit mot forelderen.
    pub fn commit(dir: &Path, rev: &str) -> Result<Self, String> {
        let root = toplevel(dir)?;
        let right = verify(&root, rev).ok_or_else(|| format!("ukjent commit: {rev}"))?;
        let left = match verify(&root, &format!("{right}^")) {
            Some(parent) => parent,
            None => empty_tree(&root)?,
        };
        Ok(Self {
            root,
            left,
            right: Some(right),
            history: None,
        })
    }

    /// PR-en mot der den gikk ut fra base-branchen, som på GitHub. Hentes fra `origin`, så
    /// repoet i `dir` må være det PR-en hører til.
    pub fn pull_request(dir: &Path, pr: &PullRequest) -> Result<Self, String> {
        let root = toplevel(dir).map_err(|_| "står ikke i et git-repo".to_owned())?;
        let origin = git(&root, &["remote", "get-url", "origin"])
            .map_err(|_| "repoet har ingen origin".to_owned())?;
        let origin = origin.trim_end();
        if !github_slug(origin).is_some_and(|s| s.eq_ignore_ascii_case(&pr.slug)) {
            return Err(format!("PR-en hører til {}, ikke {origin}", pr.slug));
        }
        let fields = ".baseRefName + \" \" + .headRefOid";
        let view = [
            "pr",
            "view",
            &pr.url,
            "--json",
            "baseRefName,headRefOid",
            "--jq",
            fields,
        ];
        // Alle branchene hentes, så fetch ikke trenger base-branchen fra gh og kan gå samtidig
        let head_ref = format!("pull/{}/head", pr.number);
        let branches = "+refs/heads/*:refs/remotes/origin/*";
        let fetch = [
            "fetch",
            "--quiet",
            "--no-write-fetch-head",
            "origin",
            &head_ref,
            branches,
        ];
        let (info, fetched) = thread::scope(|s| {
            let info = s.spawn(|| gh(&root, &view));
            let fetched = git(&root, &fetch);
            (info.join().unwrap(), fetched)
        });
        let info = info?;
        fetched?;
        let (base, head) = info
            .trim_end()
            .split_once(' ')
            .ok_or_else(|| format!("uventet svar fra gh: {info}"))?;
        let right = verify(&root, head).ok_or_else(|| format!("fant ikke {head} etter fetch"))?;
        let fork = git(&root, &["merge-base", &format!("origin/{base}"), &right])?;
        Ok(Self {
            root,
            left: fork.trim_end().to_owned(),
            right: Some(right),
            history: None,
        })
    }

    /// Working tree med filer git ikke sporer ennå, mot HEAD. Commit-lista har commitene på
    /// branchen, eller de siste når branchen ikke har egne.
    pub fn work_tree(dir: &Path) -> Result<Self, String> {
        // Hvert git-kall koster en prosess, så de uavhengige kjøres samtidig
        let (root, head, branch) = thread::scope(|s| {
            let head = s.spawn(|| verify(dir, "HEAD"));
            let branch = s.spawn(|| default_branch(dir));
            (toplevel(dir), head.join().unwrap(), branch.join().unwrap())
        });
        let root = root?;
        let Some(head) = head else {
            let left = empty_tree(&root)?;
            return Ok(Self {
                root,
                left,
                right: None,
                history: None,
            });
        };
        let fork = branch.and_then(|b| {
            let out = git(&root, &["merge-base", "HEAD", &b]).ok()?;
            Some(out.trim_end().to_owned())
        });
        let history = match fork {
            Some(fork) if fork != head => format!("{fork}..HEAD"),
            _ => "HEAD".to_owned(),
        };
        Ok(Self {
            root,
            left: head,
            right: None,
            history: Some(history),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Working tree kan endre seg mens den vises, en commit kan ikke.
    pub fn is_work_tree(&self) -> bool {
        self.right.is_none()
    }

    /// Commitene på branchen, eller de siste. Nyeste først. Tom for en commit.
    pub fn commits(&self) -> Result<Vec<Commit>, String> {
        let Some(history) = &self.history else {
            return Ok(Vec::new());
        };
        let max = format!("-n{MAX_COMMITS}");
        let args = ["log", "-z", "--format=%H%x1f%ct%x1f%s", &max, history, "--"];
        Ok(parse_log(&git(&self.root, &args)?))
    }

    /// Mappenavnet til repoet eller worktreet.
    pub fn name(&self) -> &str {
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
    }

    pub fn changes(&self) -> Result<Vec<FileChange>, String> {
        let mut args = vec!["diff", "--name-status", "-z", "-M", &self.left];
        args.extend(self.right.as_deref());
        let untracked = ["ls-files", "-z", "--others", "--exclude-standard"];
        let (diff, untracked) = thread::scope(|s| {
            let untracked = self
                .right
                .is_none()
                .then(|| s.spawn(|| git(&self.root, &untracked)));
            (git(&self.root, &args), untracked.map(|t| t.join().unwrap()))
        });
        let mut changes = parse_name_status(&diff?);
        if let Some(untracked) = untracked {
            changes.extend(untracked?.split_terminator('\0').map(|p| FileChange {
                path: p.to_owned(),
                old_path: None,
                status: Status::Added,
            }));
        }
        Ok(changes)
    }

    /// Innholdet på hver side. En side som ikke finnes er tom.
    pub fn read(&self, change: &FileChange) -> Result<[String; 2], String> {
        let left = match change.status {
            Status::Added => String::new(),
            _ => self.blob(&self.left, change.left_path())?,
        };
        let right = match (change.status, &self.right) {
            (Status::Deleted, _) => String::new(),
            (_, Some(rev)) => self.blob(rev, &change.path)?,
            (_, None) => {
                let bytes = std::fs::read(self.root.join(&change.path))
                    .map_err(|e| format!("{}: {e}", change.path))?;
                String::from_utf8_lossy(&bytes).into_owned()
            }
        };
        Ok([left, right])
    }

    /// Ruller tilbake én endring i working tree: linjene `lines[1]` erstattes med `lines[0]`
    /// fra venstre side. Nekter hvis fila ikke lenger er `before`, det diffen viser.
    /// Byttes som bytes, så encoding og linjeskift ikke endres.
    pub fn rollback(
        &self,
        change: &FileChange,
        lines: [Range<usize>; 2],
        before: &str,
    ) -> Result<String, String> {
        let path = &change.path;
        if !self.is_work_tree() {
            return Err("en commit kan ikke endres".into());
        }
        let full = self.root.join(path);
        let disk = std::fs::read(&full).map_err(|e| format!("{path}: {e}"))?;
        if String::from_utf8_lossy(&disk) != before {
            return Err(format!("{path} er endret på disk"));
        }
        let left = match change.status {
            Status::Added => Vec::new(),
            _ => self.blob_bytes(&self.left, change.left_path())?,
        };
        let after = replace_with_left(&left, &disk, lines)
            .ok_or_else(|| format!("{path}: linjene finnes ikke"))?;
        std::fs::write(&full, &after).map_err(|e| format!("{path}: {e}"))?;
        Ok(String::from_utf8_lossy(&after).into_owned())
    }

    fn blob(&self, rev: &str, path: &str) -> Result<String, String> {
        let bytes = self.blob_bytes(rev, path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Fila slik den ligger på disk etter checkout, med linjeskift fra `autocrlf` og
    /// `.gitattributes`. Ellers er hver linje endret i en fil med CRLF på disk.
    fn blob_bytes(&self, rev: &str, path: &str) -> Result<Vec<u8>, String> {
        git_bytes(
            &self.root,
            &["cat-file", "--filters", &format!("{rev}:{path}")],
        )
    }
}

fn toplevel(dir: &Path) -> Result<PathBuf, String> {
    let out = git(dir, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(out.trim_end()))
}

fn verify(root: &Path, rev: &str) -> Option<String> {
    let spec = format!("{rev}^{{commit}}");
    let out = git(root, &["rev-parse", "--verify", "--quiet", &spec]).ok()?;
    Some(out.trim_end().to_owned())
}

/// `origin` først, så commit-lista blir den samme som i PR-en. Lokal main ligger ofte bak.
fn default_branch(root: &Path) -> Option<String> {
    let remote = git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()
    .map(|s| s.trim_end().to_owned());
    [remote, Some("main".into()), Some("master".into())]
        .into_iter()
        .flatten()
        .find(|b| verify(root, b).is_some())
}

/// `eier/repo` fra en SSH- eller HTTPS-remote på GitHub.
fn github_slug(remote: &str) -> Option<&str> {
    let (_, rest) = remote.split_once("github.com")?;
    let rest = rest.strip_prefix([':', '/'])?;
    Some(rest.trim_end_matches('/').trim_end_matches(".git"))
}

/// Hashen avhenger av om repoet bruker SHA-1 eller SHA-256.
fn empty_tree(root: &Path) -> Result<String, String> {
    let out = git(root, &["hash-object", "-t", "tree", "/dev/null"])?;
    Ok(out.trim_end().to_owned())
}

/// Bytter linjene `lines[1]` i `right` med linjene `lines[0]` fra `left`. `None` hvis
/// linjene ikke finnes.
fn replace_with_left(left: &[u8], right: &[u8], lines: [Range<usize>; 2]) -> Option<Vec<u8>> {
    let left: Vec<&[u8]> = left.split(|&b| b == b'\n').collect();
    let right: Vec<&[u8]> = right.split(|&b| b == b'\n').collect();
    let [l, r] = lines;
    let before = right.get(..r.start)?;
    let replacement = left.get(l)?;
    let after = right.get(r.end..)?;
    Some([before, replacement, after].concat().join(&b'\n'))
}

/// Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_bytes(dir, args)?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// `/usr/bin/git` på macOS er en shim som gjør hvert kall flere ganger tregere, så vi
/// slår opp den ekte binæren én gang.
fn git_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        Command::new("git")
            .arg("--exec-path")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| Path::new(String::from_utf8_lossy(&o.stdout).trim_end()).join("git"))
            .filter(|p| p.exists())
            .unwrap_or_else(|| "git".into())
    })
}

fn git_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(git_bin());
    cmd.arg("-C").arg(dir).args(args);
    run(cmd, "git")
}

fn gh(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.current_dir(dir).args(args);
    Ok(String::from_utf8_lossy(&run(cmd, "gh")?).into_owned())
}

fn run(mut cmd: Command, name: &str) -> Result<Vec<u8>, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("kunne ikke kjøre {name}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim_end().to_owned());
    }
    Ok(out.stdout)
}

/// `R100\0gammel\0ny\0M\0sti\0`. Type-endring og konflikt vises som endret.
fn parse_name_status(out: &str) -> Vec<FileChange> {
    let mut fields = out.split_terminator('\0');
    let mut changes = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let change = match status.as_bytes()[0] {
            b'A' => FileChange {
                path: path.to_owned(),
                old_path: None,
                status: Status::Added,
            },
            b'D' => FileChange {
                path: path.to_owned(),
                old_path: None,
                status: Status::Deleted,
            },
            b'R' | b'C' => {
                let Some(new) = fields.next() else { break };
                FileChange {
                    path: new.to_owned(),
                    old_path: Some(path.to_owned()),
                    status: Status::Renamed,
                }
            }
            _ => FileChange {
                path: path.to_owned(),
                old_path: None,
                status: Status::Modified,
            },
        };
        changes.push(change);
    }
    changes
}

/// `hash\x1ftid\x1fmelding\0` per commit.
fn parse_log(out: &str) -> Vec<Commit> {
    out.split_terminator('\0')
        .filter_map(|entry| {
            let mut fields = entry.splitn(3, '\x1f');
            Some(Commit {
                hash: fields.next()?.to_owned(),
                time: fields.next()?.parse().ok()?,
                subject: fields.next()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRepo(PathBuf);

    impl TempRepo {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("jetdiff-git-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self(dir);
            repo.git(&["init", "-q", "-b", "main"]);
            repo.git(&["config", "user.email", "t@t"]);
            repo.git(&["config", "user.name", "t"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            git(&self.0, args).unwrap()
        }

        fn write(&self, path: &str, text: &str) {
            let p = self.0.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }

        fn commit(&self) {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", "c"]);
        }

        /// Som etter en fetch der `origin/main` står på HEAD.
        fn origin_main(&self) {
            self.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
            self.git(&[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ]);
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn subjects(repo: &Repo) -> Vec<String> {
        repo.commits()
            .unwrap()
            .into_iter()
            .map(|c| c.subject)
            .collect()
    }

    fn summary(changes: &[FileChange]) -> Vec<(Status, &str, Option<&str>)> {
        let mut s: Vec<_> = changes
            .iter()
            .map(|c| (c.status, c.path.as_str(), c.old_path.as_deref()))
            .collect();
        s.sort_by_key(|c| c.1);
        s
    }

    #[test]
    fn parser_name_status() {
        let c = parse_name_status("M\0a.rs\0R087\0gammel.rs\0ny.rs\0D\0borte.rs\0");
        assert_eq!(
            summary(&c),
            [
                (Status::Modified, "a.rs", None),
                (Status::Deleted, "borte.rs", None),
                (Status::Renamed, "ny.rs", Some("gammel.rs")),
            ]
        );
    }

    #[test]
    fn forste_commit_mot_tomt_tre() {
        let r = TempRepo::new("forste");
        r.write("src/a.rs", "fn a() {}\n");
        r.commit();
        let repo = Repo::commit(&r.0, "HEAD").unwrap();
        let changes = repo.changes().unwrap();
        assert_eq!(summary(&changes), [(Status::Added, "src/a.rs", None)]);
        assert_eq!(repo.read(&changes[0]).unwrap(), ["", "fn a() {}\n"]);
    }

    #[test]
    fn commit_med_rename_og_sletting() {
        let r = TempRepo::new("rename");
        let long = "linje\n".repeat(20);
        r.write("gammel.rs", &long);
        r.write("borte.rs", "x\n");
        r.commit();
        std::fs::rename(r.0.join("gammel.rs"), r.0.join("ny.rs")).unwrap();
        std::fs::remove_file(r.0.join("borte.rs")).unwrap();
        r.commit();
        let repo = Repo::commit(&r.0, "HEAD").unwrap();
        let changes = repo.changes().unwrap();
        assert_eq!(
            summary(&changes),
            [
                (Status::Deleted, "borte.rs", None),
                (Status::Renamed, "ny.rs", Some("gammel.rs")),
            ]
        );
        let renamed = changes
            .iter()
            .find(|c| c.status == Status::Renamed)
            .unwrap();
        assert_eq!(repo.read(renamed).unwrap(), [long.clone(), long]);
    }

    #[test]
    fn working_tree_med_nye_filer() {
        let r = TempRepo::new("worktree");
        r.write("a.rs", "gammel\n");
        r.commit();
        r.write("a.rs", "ny\n");
        r.write("ny/b.rs", "b\n");
        let repo = Repo::work_tree(&r.0).unwrap();
        let changes = repo.changes().unwrap();
        assert_eq!(
            summary(&changes),
            [
                (Status::Modified, "a.rs", None),
                (Status::Added, "ny/b.rs", None)
            ]
        );
        assert_eq!(repo.read(&changes[0]).unwrap(), ["gammel\n", "ny\n"]);
    }

    #[test]
    fn bytt_linjer() {
        let replace = |l: &str, r: &str, lines: [Range<usize>; 2]| {
            String::from_utf8(replace_with_left(l.as_bytes(), r.as_bytes(), lines).unwrap())
                .unwrap()
        };
        // Områdene er det `FileDiff` gir for disse parene
        assert_eq!(replace("a\nb\nc\n", "a\nx\nc\n", [1..2, 1..2]), "a\nb\nc\n");
        assert_eq!(replace("a\nb\n", "a\nx\nb\n", [1..1, 1..2]), "a\nb\n");
        assert_eq!(replace("a\nx\nb\n", "a\nb\n", [1..2, 1..1]), "a\nx\nb\n");
        let (l, r) = ("a\nb\nc\nd\n", "x\nb\nc\ny\n");
        assert_eq!(replace(l, r, [0..1, 0..1]), "a\nb\nc\ny\n");
        assert_eq!(replace(l, r, [3..4, 3..4]), "x\nb\nc\nd\n");
        // Slutten av fila, med og uten linjeskift
        assert_eq!(replace("a\nb", "a\nb\nc", [2..2, 2..3]), "a\nb");
        assert_eq!(replace("a\nb\nc", "a\nb", [2..3, 2..2]), "a\nb\nc");
        assert_eq!(replace("a\nb", "a\nb\n", [2..2, 2..3]), "a\nb");
        assert_eq!(
            replace("a\r\nb\r\n", "a\r\nx\r\n", [1..2, 1..2]),
            "a\r\nb\r\n"
        );
        assert_eq!(replace_with_left(b"a", b"a", [0..1, 1..2]), None);
    }

    #[test]
    fn ruller_tilbake_bare_uendret_fil() {
        let r = TempRepo::new("rollback");
        r.write("a.rs", "a\nb\n");
        r.commit();
        r.write("a.rs", "a\nx\n");
        let repo = Repo::work_tree(&r.0).unwrap();
        let change = repo.changes().unwrap().remove(0);
        assert_eq!(
            repo.rollback(&change, [1..2, 1..2], "annet\n"),
            Err("a.rs er endret på disk".into())
        );
        assert_eq!(
            repo.rollback(&change, [1..2, 1..2], "a\nx\n"),
            Ok("a\nb\n".into())
        );
        assert_eq!(std::fs::read_to_string(r.0.join("a.rs")).unwrap(), "a\nb\n");
        let head = r.git(&["rev-parse", "HEAD"]);
        let commit = Repo::commit(&r.0, head.trim_end()).unwrap();
        assert!(commit.rollback(&change, [1..2, 1..2], "a\nb\n").is_err());
    }

    #[test]
    fn latin1_beholder_bytene() {
        let r = TempRepo::new("latin1");
        std::fs::write(r.0.join("a.txt"), b"bl\xe5b\xe6r\nr\xf8d\n").unwrap();
        r.commit();
        std::fs::write(r.0.join("a.txt"), b"bl\xe5b\xe6r\nr\xf8dgr\xf8t\n").unwrap();
        let repo = Repo::work_tree(&r.0).unwrap();
        let change = repo.changes().unwrap().remove(0);
        let [_, before] = repo.read(&change).unwrap();
        repo.rollback(&change, [1..2, 1..2], &before).unwrap();
        assert_eq!(
            std::fs::read(r.0.join("a.txt")).unwrap(),
            b"bl\xe5b\xe6r\nr\xf8d\n"
        );
    }

    #[test]
    fn venstre_side_med_autocrlf() {
        let r = TempRepo::new("autocrlf");
        r.write(".gitattributes", "*.txt eol=crlf\n");
        r.write("a.txt", "a\nb\n");
        r.commit();
        r.write("a.txt", "a\r\nx\r\n");
        let repo = Repo::work_tree(&r.0).unwrap();
        let change = repo.changes().unwrap().remove(0);
        assert_eq!(repo.read(&change).unwrap()[0], "a\r\nb\r\n");
        repo.rollback(&change, [1..2, 1..2], "a\r\nx\r\n").unwrap();
        assert_eq!(std::fs::read(r.0.join("a.txt")).unwrap(), b"a\r\nb\r\n");
    }

    #[test]
    fn branch_viser_bare_det_som_ikke_er_committet() {
        let r = TempRepo::new("branch");
        r.write("a.rs", "a\n");
        r.commit();
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.write("b.rs", "b\n");
        r.commit();
        r.write("a.rs", "endret\n");
        let repo = Repo::work_tree(&r.0).unwrap();
        assert_eq!(
            summary(&repo.changes().unwrap()),
            [(Status::Modified, "a.rs", None)]
        );
    }

    #[test]
    fn commits_mot_origin_foran_lokal_main() {
        let r = TempRepo::new("origin");
        r.write("a.rs", "a\n");
        r.commit();
        // Andres endring er merget til origin/main, men lokal main er ikke pullet
        r.git(&["checkout", "-q", "-b", "andre"]);
        r.write("c.rs", "c\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "andres"]);
        r.origin_main();
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.write("b.rs", "b\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "min"]);
        assert_eq!(subjects(&Repo::work_tree(&r.0).unwrap()), ["min"]);
    }

    #[test]
    fn main_med_upushede_commits() {
        let r = TempRepo::new("upushet");
        r.write("a.rs", "a\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "pushet"]);
        r.origin_main();
        r.write("b.rs", "b\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "upushet"]);
        assert_eq!(subjects(&Repo::work_tree(&r.0).unwrap()), ["upushet"]);
    }

    #[test]
    fn commits_pa_branchen_ellers_de_siste() {
        let r = TempRepo::new("commits");
        assert!(Repo::work_tree(&r.0).unwrap().commits().unwrap().is_empty());
        r.write("a.rs", "a\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "første"]);
        r.write("a.rs", "b\n");
        r.git(&["commit", "-q", "-am", "andre\n\nmer tekst"]);
        assert_eq!(
            subjects(&Repo::work_tree(&r.0).unwrap()),
            ["andre", "første"]
        );
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.write("a.rs", "c\n");
        r.git(&["commit", "-q", "-am", "på branchen"]);
        assert_eq!(subjects(&Repo::work_tree(&r.0).unwrap()), ["på branchen"]);
    }

    #[test]
    fn parser_log() {
        // Resten av linja er meldingen, også \x1f. Ødelagte oppføringer hoppes over.
        let c = parse_log("abc\x1f1700000000\x1fen: a\x1fb\0ødelagt\0def\x1f1\x1fto\0");
        let commit = |hash: &str, time, subject: &str| Commit {
            hash: hash.into(),
            subject: subject.into(),
            time,
        };
        assert_eq!(
            c,
            [
                commit("abc", 1_700_000_000, "en: a\x1fb"),
                commit("def", 1, "to")
            ]
        );
    }

    #[test]
    fn parser_pr_lenke() {
        let pr = PullRequest::parse("https://github.com/eier/repo/pull/12/files#diff-1").unwrap();
        assert_eq!((pr.slug.as_str(), pr.number), ("eier/repo", 12));
        assert!(PullRequest::parse("https://github.com/eier/repo/issues/12").is_none());
        assert!(PullRequest::parse("https://github.com/eier/repo/pull/").is_none());
        assert!(PullRequest::parse("https://gitlab.com/eier/repo/pull/12").is_none());
    }

    #[test]
    fn slug_fra_remote() {
        for remote in [
            "git@github.com:eier/repo.git",
            "ssh://git@github.com/eier/repo.git",
            "https://github.com/eier/repo",
        ] {
            assert_eq!(github_slug(remote), Some("eier/repo"), "{remote}");
        }
        assert_eq!(github_slug("git@gitlab.com:eier/repo.git"), None);
    }

    #[test]
    fn pr_fra_annet_repo_gir_feil() {
        let r = TempRepo::new("pr");
        let pr = PullRequest::parse("https://github.com/eier/repo/pull/1").unwrap();
        let err = |r: &TempRepo| Repo::pull_request(&r.0, &pr).err().unwrap();
        assert_eq!(err(&r), "repoet har ingen origin");
        r.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/eier/annet.git",
        ]);
        assert_eq!(
            err(&r),
            "PR-en hører til eier/repo, ikke https://github.com/eier/annet.git"
        );
    }

    #[test]
    fn ukjent_commit_gir_feil() {
        let r = TempRepo::new("ukjent");
        r.write("a", "a");
        r.commit();
        assert!(Repo::commit(&r.0, "finnes-ikke").is_err());
    }
}
