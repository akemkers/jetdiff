//! Endringer og filinnhold fra git. Kaller `git` så resultatet blir det samme som i
//! `git diff`, med brukerens config og rename-deteksjon.

use std::path::{Path, PathBuf};
use std::process::Command;

use smudiff_changes::{FileChange, Status};

pub struct Repo {
    root: PathBuf,
    /// Venstre side. Det tomme treet hvis det ikke finnes noen forelder.
    left: String,
    /// `None` er working tree.
    right: Option<String>,
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
        })
    }

    /// Working tree mot HEAD, med filer git ikke sporer ennå.
    pub fn work_tree(dir: &Path) -> Result<Self, String> {
        let root = toplevel(dir)?;
        let left = match verify(&root, "HEAD") {
            Some(head) => head,
            None => empty_tree(&root)?,
        };
        Ok(Self {
            root,
            left,
            right: None,
        })
    }

    pub fn changes(&self) -> Result<Vec<FileChange>, String> {
        let mut args = vec!["diff", "--name-status", "-z", "-M", &self.left];
        args.extend(self.right.as_deref());
        let mut changes = parse_name_status(&git(&self.root, &args)?);
        if self.right.is_none() {
            let untracked = git(
                &self.root,
                &["ls-files", "-z", "--others", "--exclude-standard"],
            )?;
            changes.extend(untracked.split_terminator('\0').map(|p| FileChange {
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

    fn blob(&self, rev: &str, path: &str) -> Result<String, String> {
        git(&self.root, &["cat-file", "blob", &format!("{rev}:{path}")])
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

/// Hashen avhenger av om repoet bruker SHA-1 eller SHA-256.
fn empty_tree(root: &Path) -> Result<String, String> {
    let out = git(root, &["hash-object", "-t", "tree", "/dev/null"])?;
    Ok(out.trim_end().to_owned())
}

/// Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("kunne ikke kjøre git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim_end().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRepo(PathBuf);

    impl TempRepo {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("smudiff-git-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self(dir);
            repo.git(&["init", "-q"]);
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
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
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
    fn ukjent_commit_gir_feil() {
        let r = TempRepo::new("ukjent");
        r.write("a", "a");
        r.commit();
        assert!(Repo::commit(&r.0, "finnes-ikke").is_err());
    }
}
