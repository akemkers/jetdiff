//! Changed files and their content from git. Calls `git` so the result is the same as in
//! `git diff`, with the user's config and rename detection.

mod branch;
mod git_dirs;
#[cfg(test)]
mod temp_repo;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;

use jetdiff_files::{ChangedFile, FileStatus};

use branch::BranchLog;
pub use git_dirs::GitDirs;

/// The commits in the commit list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    pub commits: Vec<Commit>,
    /// The branch's own commits. On the base branch without any, the newest commits on HEAD, so older
    /// ones can be looked at.
    pub own: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    /// The first line of the message.
    pub subject: String,
    /// Seconds since 1970.
    pub time: i64,
}

/// A link like `https://github.com/owner/repo/pull/12`.
#[derive(Debug, PartialEq, Eq)]
pub struct PullRequest {
    pub url: String,
    /// `owner/repo`.
    pub slug: String,
    pub number: u32,
}

impl PullRequest {
    /// Also accepts links to a tab or comment in the PR.
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

#[derive(Clone)]
pub struct Repo {
    root: PathBuf,
    /// The revision on Left. The empty tree if there is no parent.
    left_side: String,
    /// The revision on Right. `None` is the working tree.
    right_side: Option<String>,
    /// Only Local changes, so rollback always means back to HEAD.
    can_rollback: bool,
    /// What the commit list shows. Only for Branch.
    branch_log: Option<BranchLog>,
    /// Only for Branch, read with the rest.
    branch_name: Option<String>,
}

impl Repo {
    /// A commit against its parent.
    pub fn commit(dir: &Path, rev: &str) -> Result<Self, String> {
        let root = toplevel(dir)?;
        let right_side = verify(&root, rev).ok_or_else(|| format!("unknown commit: {rev}"))?;
        let left_side = match verify(&root, &format!("{right_side}^")) {
            Some(parent) => parent,
            None => empty_tree(&root)?,
        };
        Ok(Self {
            root,
            left_side,
            right_side: Some(right_side),
            can_rollback: false,
            branch_log: None,
            branch_name: None,
        })
    }

    /// The PR against where it branched off the base branch, as on GitHub. Fetched from `origin`, so
    /// the repo in `dir` must be the one the PR belongs to.
    pub fn pull_request(dir: &Path, pr: &PullRequest) -> Result<Self, String> {
        let root = toplevel(dir).map_err(|_| "not in a git repo".to_owned())?;
        // As configured, since `get-url` applies `insteadOf` and can hide that it is GitHub
        let origin = git(&root, &["config", "--get", "remote.origin.url"])
            .map_err(|_| "the repo has no origin".to_owned())?;
        let origin = origin.trim_end();
        if !github_slug(origin).is_some_and(|s| s.eq_ignore_ascii_case(&pr.slug)) {
            return Err(format!("the PR belongs to {}, not {origin}", pr.slug));
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
        // All the branches are fetched, so fetch doesn't need the base branch from gh and can run at the same time
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
            .ok_or_else(|| format!("unexpected answer from gh: {info}"))?;
        let right_side =
            verify(&root, head).ok_or_else(|| format!("did not find {head} after fetch"))?;
        let fork = git(
            &root,
            &["merge-base", &format!("origin/{base}"), &right_side],
        )?;
        Ok(Self {
            root,
            left_side: fork.trim_end().to_owned(),
            right_side: Some(right_side),
            can_rollback: false,
            branch_log: None,
            branch_name: None,
        })
    }

    /// Local changes, with files git doesn't track yet, against HEAD.
    pub fn local_changes(dir: &Path) -> Result<Self, String> {
        // Each git call costs a process, so the independent ones run at the same time
        let (root, head) = thread::scope(|s| {
            let head = s.spawn(|| verify(dir, "HEAD"));
            (toplevel(dir), head.join().unwrap())
        });
        let root = root?;
        let left_side = match head {
            Some(head) => head,
            None => empty_tree(&root)?,
        };
        Ok(Self {
            root,
            left_side,
            right_side: None,
            can_rollback: true,
            branch_log: None,
            branch_name: None,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Branch and Local changes can change while they are shown; a commit can't.
    pub fn reads_working_tree(&self) -> bool {
        self.right_side.is_none()
    }

    pub fn can_rollback(&self) -> bool {
        self.can_rollback
    }

    /// The folder name of the repo or worktree.
    pub fn name(&self) -> &str {
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
    }

    pub fn changed_files(&self) -> Result<Vec<ChangedFile>, String> {
        let mut args = vec!["diff", "--name-status", "-z", "-M", &self.left_side];
        args.extend(self.right_side.as_deref());
        let untracked = ["ls-files", "-z", "--others", "--exclude-standard"];
        let (diff, untracked) = thread::scope(|s| {
            let untracked = self
                .right_side
                .is_none()
                .then(|| s.spawn(|| git(&self.root, &untracked)));
            (git(&self.root, &args), untracked.map(|t| t.join().unwrap()))
        });
        let mut files = parse_name_status(&diff?);
        if let Some(untracked) = untracked {
            files.extend(untracked?.split_terminator('\0').map(|path| ChangedFile {
                path: path.to_owned(),
                renamed_from: None,
                status: FileStatus::Added,
            }));
        }
        Ok(files)
    }

    /// The content on each side. A side that doesn't exist is empty.
    pub fn read(&self, file: &ChangedFile) -> Result<[String; 2], String> {
        let left_side = match file.status {
            FileStatus::Added => String::new(),
            _ => self.blob(&self.left_side, file.left_side_path())?,
        };
        let right_side = match (file.status, &self.right_side) {
            (FileStatus::Deleted, _) => String::new(),
            (_, Some(rev)) => self.blob(rev, &file.path)?,
            (_, None) => {
                let bytes = std::fs::read(self.root.join(&file.path))
                    .map_err(|e| format!("{}: {e}", file.path))?;
                String::from_utf8_lossy(&bytes).into_owned()
            }
        };
        Ok([left_side, right_side])
    }

    /// Rolls back one hunk in Local changes: the lines `lines[1]` are replaced with `lines[0]`
    /// from Left. Refuses if the file is no longer `before`, what the diff shows.
    /// Replaced as bytes, so encoding and line breaks don't change.
    pub fn rollback(
        &self,
        file: &ChangedFile,
        lines: [Range<usize>; 2],
        before: &str,
    ) -> Result<String, String> {
        let path = &file.path;
        if !self.can_rollback {
            return Err("only Local changes can be rolled back".into());
        }
        let full = self.root.join(path);
        let disk = std::fs::read(&full).map_err(|e| format!("{path}: {e}"))?;
        if String::from_utf8_lossy(&disk) != before {
            return Err(format!("{path} has changed on disk"));
        }
        let left_side = match file.status {
            FileStatus::Added => Vec::new(),
            _ => self.blob_bytes(&self.left_side, file.left_side_path())?,
        };
        let after = replace_with_left_side(&left_side, &disk, lines)
            .ok_or_else(|| format!("{path}: the lines don't exist"))?;
        std::fs::write(&full, &after).map_err(|e| format!("{path}: {e}"))?;
        Ok(String::from_utf8_lossy(&after).into_owned())
    }

    fn blob(&self, rev: &str, path: &str) -> Result<String, String> {
        let bytes = self.blob_bytes(rev, path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The file as it is on disk after checkout, with line breaks from `autocrlf` and
    /// `.gitattributes`. Otherwise every line is changed in a file with CRLF on disk.
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

/// `owner/repo` from an SSH or HTTPS remote on GitHub.
fn github_slug(remote: &str) -> Option<&str> {
    let (_, rest) = remote.split_once("github.com")?;
    let rest = rest.strip_prefix([':', '/'])?;
    Some(rest.trim_end_matches('/').trim_end_matches(".git"))
}

/// The hash depends on whether the repo uses SHA-1 or SHA-256.
fn empty_tree(root: &Path) -> Result<String, String> {
    let out = git(root, &["hash-object", "-t", "tree", "/dev/null"])?;
    Ok(out.trim_end().to_owned())
}

/// Replaces the lines `lines[1]` in `right_side` with the lines `lines[0]` from `left_side`.
/// `None` if the lines don't exist.
fn replace_with_left_side(
    left_side: &[u8],
    right_side: &[u8],
    lines: [Range<usize>; 2],
) -> Option<Vec<u8>> {
    let left_side: Vec<&[u8]> = left_side.split(|&b| b == b'\n').collect();
    let right_side: Vec<&[u8]> = right_side.split(|&b| b == b'\n').collect();
    let [left_side_lines, right_side_lines] = lines;
    let before = right_side.get(..right_side_lines.start)?;
    let replacement = left_side.get(left_side_lines)?;
    let after = right_side.get(right_side_lines.end..)?;
    Some([before, replacement, after].concat().join(&b'\n'))
}

/// Binary files and wrong encoding are shown with replacement characters instead of failing.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_bytes(dir, args)?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// `/usr/bin/git` on macOS is a shim that makes each call several times slower, so we
/// look up the real binary once.
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
        .map_err(|e| format!("could not run {name}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim_end().to_owned());
    }
    Ok(out.stdout)
}

/// `R100\0old\0new\0M\0path\0`. Type changes and conflicts are shown as modified.
fn parse_name_status(out: &str) -> Vec<ChangedFile> {
    let mut fields = out.split_terminator('\0');
    let mut files = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let file = match status.as_bytes()[0] {
            b'A' => ChangedFile {
                path: path.to_owned(),
                renamed_from: None,
                status: FileStatus::Added,
            },
            b'D' => ChangedFile {
                path: path.to_owned(),
                renamed_from: None,
                status: FileStatus::Deleted,
            },
            b'R' | b'C' => {
                let Some(renamed_to) = fields.next() else {
                    break;
                };
                ChangedFile {
                    path: renamed_to.to_owned(),
                    renamed_from: Some(path.to_owned()),
                    status: FileStatus::Renamed,
                }
            }
            _ => ChangedFile {
                path: path.to_owned(),
                renamed_from: None,
                status: FileStatus::Modified,
            },
        };
        files.push(file);
    }
    files
}

/// `hash\x1ftime\x1fmessage\0` per commit.
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
    use crate::temp_repo::TempRepo;

    pub(crate) fn summary(files: &[ChangedFile]) -> Vec<(FileStatus, &str, Option<&str>)> {
        let mut summary: Vec<_> = files
            .iter()
            .map(|file| {
                (
                    file.status,
                    file.path.as_str(),
                    file.renamed_from.as_deref(),
                )
            })
            .collect();
        summary.sort_by_key(|&(_, path, _)| path);
        summary
    }

    #[test]
    fn parses_name_status() {
        let c = parse_name_status("M\0a.rs\0R087\0old.rs\0new.rs\0D\0gone.rs\0");
        assert_eq!(
            summary(&c),
            [
                (FileStatus::Modified, "a.rs", None),
                (FileStatus::Deleted, "gone.rs", None),
                (FileStatus::Renamed, "new.rs", Some("old.rs")),
            ]
        );
    }

    #[test]
    fn first_commit_against_empty_tree() {
        let r = TempRepo::new("first");
        r.write("src/a.rs", "fn a() {}\n");
        r.commit();
        let repo = Repo::commit(&r.0, "HEAD").unwrap();
        let files = repo.changed_files().unwrap();
        assert_eq!(summary(&files), [(FileStatus::Added, "src/a.rs", None)]);
        assert_eq!(repo.read(&files[0]).unwrap(), ["", "fn a() {}\n"]);
    }

    #[test]
    fn commit_with_rename_and_deletion() {
        let r = TempRepo::new("rename");
        let long = "line\n".repeat(20);
        r.write("old.rs", &long);
        r.write("gone.rs", "x\n");
        r.commit();
        std::fs::rename(r.0.join("old.rs"), r.0.join("new.rs")).unwrap();
        std::fs::remove_file(r.0.join("gone.rs")).unwrap();
        r.commit();
        let repo = Repo::commit(&r.0, "HEAD").unwrap();
        let files = repo.changed_files().unwrap();
        assert_eq!(
            summary(&files),
            [
                (FileStatus::Deleted, "gone.rs", None),
                (FileStatus::Renamed, "new.rs", Some("old.rs")),
            ]
        );
        let renamed = files
            .iter()
            .find(|file| file.status == FileStatus::Renamed)
            .unwrap();
        assert_eq!(repo.read(renamed).unwrap(), [long.clone(), long]);
    }

    #[test]
    fn local_changes_with_new_files() {
        let r = TempRepo::new("local-changes");
        r.write("a.rs", "old\n");
        r.commit();
        r.write("a.rs", "new\n");
        r.write("new/b.rs", "b\n");
        let repo = Repo::local_changes(&r.0).unwrap();
        let files = repo.changed_files().unwrap();
        assert_eq!(
            summary(&files),
            [
                (FileStatus::Modified, "a.rs", None),
                (FileStatus::Added, "new/b.rs", None)
            ]
        );
        assert_eq!(repo.read(&files[0]).unwrap(), ["old\n", "new\n"]);
    }

    #[test]
    fn replace_lines() {
        let replace = |l: &str, r: &str, lines: [Range<usize>; 2]| {
            String::from_utf8(replace_with_left_side(l.as_bytes(), r.as_bytes(), lines).unwrap())
                .unwrap()
        };
        // The ranges are what `FileDiff` gives for these pairs
        assert_eq!(replace("a\nb\nc\n", "a\nx\nc\n", [1..2, 1..2]), "a\nb\nc\n");
        assert_eq!(replace("a\nb\n", "a\nx\nb\n", [1..1, 1..2]), "a\nb\n");
        assert_eq!(replace("a\nx\nb\n", "a\nb\n", [1..2, 1..1]), "a\nx\nb\n");
        let (l, r) = ("a\nb\nc\nd\n", "x\nb\nc\ny\n");
        assert_eq!(replace(l, r, [0..1, 0..1]), "a\nb\nc\ny\n");
        assert_eq!(replace(l, r, [3..4, 3..4]), "x\nb\nc\nd\n");
        // The end of the file, with and without a line break
        assert_eq!(replace("a\nb", "a\nb\nc", [2..2, 2..3]), "a\nb");
        assert_eq!(replace("a\nb\nc", "a\nb", [2..3, 2..2]), "a\nb\nc");
        assert_eq!(replace("a\nb", "a\nb\n", [2..2, 2..3]), "a\nb");
        assert_eq!(
            replace("a\r\nb\r\n", "a\r\nx\r\n", [1..2, 1..2]),
            "a\r\nb\r\n"
        );
        assert_eq!(replace_with_left_side(b"a", b"a", [0..1, 1..2]), None);
    }

    #[test]
    fn rolls_back_only_unchanged_file() {
        let r = TempRepo::new("rollback");
        r.write("a.rs", "a\nb\n");
        r.commit();
        r.write("a.rs", "a\nx\n");
        let repo = Repo::local_changes(&r.0).unwrap();
        let file = repo.changed_files().unwrap().remove(0);
        assert_eq!(
            repo.rollback(&file, [1..2, 1..2], "other\n"),
            Err("a.rs has changed on disk".into())
        );
        assert_eq!(
            repo.rollback(&file, [1..2, 1..2], "a\nx\n"),
            Ok("a\nb\n".into())
        );
        assert_eq!(std::fs::read_to_string(r.0.join("a.rs")).unwrap(), "a\nb\n");
        let head = r.git(&["rev-parse", "HEAD"]);
        let commit = Repo::commit(&r.0, head.trim_end()).unwrap();
        assert!(commit.rollback(&file, [1..2, 1..2], "a\nb\n").is_err());
    }

    #[test]
    fn latin1_keeps_the_bytes() {
        let r = TempRepo::new("latin1");
        std::fs::write(r.0.join("a.txt"), b"bl\xe5b\xe6r\nr\xf8d\n").unwrap();
        r.commit();
        std::fs::write(r.0.join("a.txt"), b"bl\xe5b\xe6r\nr\xf8dgr\xf8t\n").unwrap();
        let repo = Repo::local_changes(&r.0).unwrap();
        let file = repo.changed_files().unwrap().remove(0);
        let [_, before] = repo.read(&file).unwrap();
        repo.rollback(&file, [1..2, 1..2], &before).unwrap();
        assert_eq!(
            std::fs::read(r.0.join("a.txt")).unwrap(),
            b"bl\xe5b\xe6r\nr\xf8d\n"
        );
    }

    #[test]
    fn left_side_with_autocrlf() {
        let r = TempRepo::new("autocrlf");
        r.write(".gitattributes", "*.txt eol=crlf\n");
        r.write("a.txt", "a\nb\n");
        r.commit();
        r.write("a.txt", "a\r\nx\r\n");
        let repo = Repo::local_changes(&r.0).unwrap();
        let file = repo.changed_files().unwrap().remove(0);
        assert_eq!(repo.read(&file).unwrap()[0], "a\r\nb\r\n");
        repo.rollback(&file, [1..2, 1..2], "a\r\nx\r\n").unwrap();
        assert_eq!(std::fs::read(r.0.join("a.txt")).unwrap(), b"a\r\nb\r\n");
    }

    #[test]
    fn branch_shows_only_what_is_not_committed() {
        let r = TempRepo::new("branch");
        r.write("a.rs", "a\n");
        r.commit();
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.write("b.rs", "b\n");
        r.commit();
        r.write("a.rs", "changed\n");
        let repo = Repo::local_changes(&r.0).unwrap();
        assert_eq!(
            summary(&repo.changed_files().unwrap()),
            [(FileStatus::Modified, "a.rs", None)]
        );
    }

    #[test]
    fn parses_log() {
        // The rest of the line is the message, also \x1f. Broken entries are skipped.
        let c = parse_log("abc\x1f1700000000\x1fone: a\x1fb\0broken\0def\x1f1\x1ftwo\0");
        let commit = |hash: &str, time, subject: &str| Commit {
            hash: hash.into(),
            subject: subject.into(),
            time,
        };
        assert_eq!(
            c,
            [
                commit("abc", 1_700_000_000, "one: a\x1fb"),
                commit("def", 1, "two")
            ]
        );
    }

    #[test]
    fn parses_pr_link() {
        let pr = PullRequest::parse("https://github.com/owner/repo/pull/12/files#diff-1").unwrap();
        assert_eq!((pr.slug.as_str(), pr.number), ("owner/repo", 12));
        assert!(PullRequest::parse("https://github.com/owner/repo/issues/12").is_none());
        assert!(PullRequest::parse("https://github.com/owner/repo/pull/").is_none());
        assert!(PullRequest::parse("https://gitlab.com/owner/repo/pull/12").is_none());
    }

    #[test]
    fn slug_from_remote() {
        for remote in [
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo.git",
            "https://github.com/owner/repo",
        ] {
            assert_eq!(github_slug(remote), Some("owner/repo"), "{remote}");
        }
        assert_eq!(github_slug("git@gitlab.com:owner/repo.git"), None);
    }

    #[test]
    fn pr_from_other_repo_fails() {
        let r = TempRepo::new("pr");
        let pr = PullRequest::parse("https://github.com/owner/repo/pull/1").unwrap();
        let err = |r: &TempRepo| Repo::pull_request(&r.0, &pr).err().unwrap();
        assert_eq!(err(&r), "the repo has no origin");
        r.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/other.git",
        ]);
        assert_eq!(
            err(&r),
            "the PR belongs to owner/repo, not https://github.com/owner/other.git"
        );
    }

    #[test]
    fn unknown_commit_fails() {
        let r = TempRepo::new("unknown");
        r.write("a", "a");
        r.commit();
        assert!(Repo::commit(&r.0, "missing").is_err());
    }
}
