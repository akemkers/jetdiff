//! The changes in a repo and the file being shown.

use std::path::PathBuf;
use std::rc::Rc;

use jetdiff_changes::{ChangeTree, FileChange};
use jetdiff_git::{Commit, Repo};
use jetdiff_viewmodel::FileDiff;

use crate::actions::RollbackRequest;
use crate::content::{Notice, build_diff};

pub struct Loaded {
    commit: Option<String>,
    repo: Repo,
    changes: Vec<FileChange>,
    commits: Vec<Commit>,
}

pub struct ReloadJob {
    root: PathBuf,
    commit: Option<String>,
}

impl ReloadJob {
    /// The commit list is fetched again every time, since Local changes decide which commits it has.
    pub fn run(self) -> Result<Loaded, String> {
        let home = Repo::work_tree(&self.root)?;
        let commits = home.commits()?;
        let repo = match &self.commit {
            Some(hash) => Repo::commit(&self.root, hash)?,
            None => home,
        };
        let changes = repo.changes()?;
        Ok(Loaded {
            commit: self.commit,
            repo,
            changes,
            commits,
        })
    }
}

pub enum Reloaded {
    Failed {
        shown: Option<String>,
        error: String,
    },
    Done {
        commits: Vec<Commit>,
        /// Only when the list has changed.
        changes: Option<Rc<[FileChange]>>,
        file: Option<usize>,
        notice: Option<Notice>,
    },
}

pub enum Show {
    Empty(Notice),
    Unchanged,
    Diff(Box<Shown>),
}

pub struct Shown {
    pub diff: FileDiff,
    pub notice: Option<Notice>,
    pub paths: [String; 2],
    /// Only Local changes can be written to, not a commit.
    pub can_rollback: bool,
    pub same_file: bool,
}

pub struct Changes {
    repo: Repo,
    commit: Option<String>,
    /// The commit `changes` is from. Becomes equal to `commit` when loading is done.
    loaded_commit: Option<String>,
    changes: Rc<[FileChange]>,
    /// What the diff shows now. The same content after a reload leaves the position alone.
    shown: Option<(FileChange, Result<[String; 2], String>)>,
}

impl Changes {
    pub fn new(repo: Repo, changes: Vec<FileChange>) -> Self {
        Self {
            repo,
            commit: None,
            loaded_commit: None,
            changes: changes.into(),
            shown: None,
        }
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn changes(&self) -> &Rc<[FileChange]> {
        &self.changes
    }

    pub fn shown(&self) -> Option<&FileChange> {
        self.shown.as_ref().map(|(c, _)| c)
    }

    pub fn first(&self) -> Option<usize> {
        ChangeTree::new(&self.changes).first()
    }

    /// Shown after the next `reload`.
    pub fn select_commit(&mut self, hash: Option<String>) {
        self.commit = hash;
    }

    pub fn reload(&self) -> ReloadJob {
        ReloadJob {
            root: self.repo.root().to_owned(),
            commit: self.commit.clone(),
        }
    }

    /// `paths` are the files the watcher saw change. Empty when the reload came from something else.
    pub fn reloaded(&mut self, loaded: Result<Loaded, String>, paths: &[String]) -> Reloaded {
        let Loaded {
            commit,
            repo,
            changes,
            commits,
        } = match loaded {
            Ok(l) => l,
            Err(error) => {
                self.commit = self.loaded_commit.clone();
                return Reloaded::Failed {
                    shown: self.commit.clone(),
                    error,
                };
            }
        };
        // Only Local changes against Local changes is an update. A commit doesn't change.
        let same_work_tree = commit.is_none() && self.loaded_commit.is_none();
        let touched = if same_work_tree && !paths.is_empty() {
            jetdiff_changes::touched(&self.changes, &changes, paths)
        } else {
            0
        };
        let selected = self.shown().map(|c| c.path.clone());
        self.repo = repo;
        self.loaded_commit = commit;
        let changed = (*changes != *self.changes).then(|| {
            self.changes = changes.into();
            self.changes.clone()
        });
        // The selected file is kept as long as it is included
        let file = selected
            .and_then(|p| self.changes.iter().position(|c| c.path == p))
            .or_else(|| self.first());
        let notice = match touched {
            0 => None,
            1 => Some(Notice::info("Updated: 1 file changed")),
            n => Some(Notice::info(format!("Updated: {n} files changed"))),
        };
        Reloaded::Done {
            commits,
            changes: changed,
            file,
            notice,
        }
    }

    pub fn load(&mut self, file: Option<usize>) -> Show {
        let Some(change) = file.map(|i| self.changes[i].clone()) else {
            self.shown = None;
            return Show::Empty(Notice::info("No changes"));
        };
        let content = self.repo.read(&change);
        self.show(change, content)
    }

    fn show(&mut self, change: FileChange, content: Result<[String; 2], String>) -> Show {
        let shown = Some((change.clone(), content.clone()));
        if shown == self.shown {
            return Show::Unchanged;
        }
        let same_file = self.shown().is_some_and(|c| c.path == change.path);
        self.shown = shown;
        let (diff, notice) = build_diff(&change, content);
        Show::Diff(Box::new(Shown {
            diff,
            notice,
            paths: [change.left_path().to_owned(), change.path.clone()],
            can_rollback: self.repo.is_work_tree(),
            same_file,
        }))
    }

    /// Run a `reload` afterwards, so the list keeps up.
    pub fn rollback(&mut self, request: &RollbackRequest) -> Option<Result<Show, String>> {
        let Some((change, Ok([left, _]))) = self.shown.clone() else {
            return None;
        };
        Some(
            self.repo
                .rollback(&change, request.lines.clone(), &request.before)
                .map(|after| self.show(change, Ok([left, after]))),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    struct TempRepo(PathBuf);

    impl TempRepo {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("jetdiff-app-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self(dir);
            repo.git(&["init", "-q", "-b", "main"]);
            repo.git(&["config", "user.email", "t@t"]);
            repo.git(&["config", "user.name", "t"]);
            repo
        }

        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .args(args)
                .current_dir(&self.0)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn write(&self, path: &str, text: &str) {
            std::fs::write(self.0.join(path), text).unwrap();
        }

        fn changes(&self) -> Changes {
            let repo = Repo::work_tree(Path::new(&self.0)).unwrap();
            let changes = repo.changes().unwrap();
            Changes::new(repo, changes)
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn reload(c: &mut Changes, paths: &[&str]) -> Reloaded {
        let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let loaded = c.reload().run();
        c.reloaded(loaded, &paths)
    }

    fn path(c: &Changes, file: Option<usize>) -> &str {
        &c.changes()[file.unwrap()].path
    }

    #[test]
    fn reload_keeps_selected_file() {
        let r = TempRepo::new("keeps");
        r.write("a.rs", "a\n");
        r.write("b.rs", "b\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "c"]);
        r.write("a.rs", "a2\n");
        r.write("b.rs", "b2\n");
        let mut c = r.changes();
        let b = c.changes().iter().position(|c| c.path == "b.rs");
        assert!(matches!(c.load(b), Show::Diff(s) if !s.same_file && s.can_rollback));
        assert!(matches!(c.load(b), Show::Unchanged));

        r.write("b.rs", "b3\n");
        r.write("c.rs", "c\n");
        let Reloaded::Done {
            changes,
            file,
            notice,
            ..
        } = reload(&mut c, &["b.rs", "c.rs"])
        else {
            panic!("reload failed");
        };
        assert!(changes.is_some());
        assert_eq!(path(&c, file), "b.rs");
        assert_eq!(notice, Some(Notice::info("Updated: 2 files changed")));
        assert!(matches!(c.load(file), Show::Diff(s) if s.same_file));
    }

    #[test]
    fn unknown_commit_shows_what_is_shown() {
        let r = TempRepo::new("unknown");
        r.write("a.rs", "a\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "c"]);
        let mut c = r.changes();
        c.select_commit(Some("missing".into()));
        assert!(matches!(
            reload(&mut c, &[]),
            Reloaded::Failed { shown: None, .. }
        ));
        assert!(matches!(
            reload(&mut c, &[]),
            Reloaded::Done { file: None, .. }
        ));
        assert!(matches!(c.load(None), Show::Empty(_)));
    }
}
