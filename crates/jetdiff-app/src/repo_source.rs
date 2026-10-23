//! A repo source: its changed files, the file being shown, and reading it again.

mod read;
mod show;

use std::rc::Rc;

use jetdiff_files::{ChangedFile, Tree};
use jetdiff_git::Repo;

use crate::actions::{ONLY_LOCAL_CHANGES, RollbackRequest};
use crate::content::build_diff;
use crate::notification::Notification;
use read::Trigger;
pub use read::{ReadJob, RepoRead};
pub use show::{Applied, Show, Shown};

pub struct RepoSource {
    repo: Repo,
    commit: Option<String>,
    /// The commit `changed_files` is from. Becomes equal to `commit` when a read is applied.
    last_read_commit: Option<String>,
    changed_files: Rc<[ChangedFile]>,
    /// What the diff shows now. The same content after a read leaves the position alone.
    shown: Option<(ChangedFile, Result<[String; 2], String>)>,
}

impl RepoSource {
    pub fn new(repo: Repo, changed_files: Vec<ChangedFile>) -> Self {
        Self {
            repo,
            commit: None,
            last_read_commit: None,
            changed_files: changed_files.into(),
            shown: None,
        }
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn changed_files(&self) -> &Rc<[ChangedFile]> {
        &self.changed_files
    }

    pub fn shown(&self) -> Option<&ChangedFile> {
        self.shown.as_ref().map(|(file, _)| file)
    }

    pub fn first(&self) -> Option<usize> {
        Tree::new(&self.changed_files).first()
    }

    /// `paths` are the files the watcher saw change.
    pub fn files_changed(&self, paths: Vec<String>) -> ReadJob {
        self.read(Trigger::FilesChanged(paths))
    }

    /// `None` is Local changes. Shown when the read is applied.
    pub fn select_commit(&mut self, hash: Option<String>) -> ReadJob {
        self.commit = hash;
        self.read(Trigger::CommitSelected)
    }

    /// After `rollback`, so the list keeps up.
    pub fn rolled_back(&self) -> ReadJob {
        self.read(Trigger::RolledBack)
    }

    fn read(&self, trigger: Trigger) -> ReadJob {
        ReadJob {
            root: self.repo.root().to_owned(),
            commit: self.commit.clone(),
            trigger,
        }
    }

    pub fn apply(&mut self, read: Result<RepoRead, String>) -> Applied {
        let RepoRead {
            trigger,
            commit,
            repo,
            changed_files,
            commits,
        } = match read {
            Ok(read) => read,
            Err(error) => {
                self.commit = self.last_read_commit.clone();
                return Applied::Failed {
                    selected_commit: self.commit.clone(),
                    brief: Notification::warning(error),
                };
            }
        };
        // Files are only counted when Local changes are read again as Local changes. A commit doesn't change.
        let same_local_changes = commit.is_none() && self.last_read_commit.is_none();
        let touched = match &trigger {
            Trigger::FilesChanged(paths) if same_local_changes => {
                jetdiff_files::touched(&self.changed_files, &changed_files, paths)
            }
            Trigger::FilesChanged(_) | Trigger::CommitSelected | Trigger::RolledBack => 0,
        };
        let selected = self.shown().map(|file| file.path.clone());
        self.repo = repo;
        self.last_read_commit = commit;
        let changed = (*changed_files != *self.changed_files).then(|| {
            self.changed_files = changed_files.into();
            self.changed_files.clone()
        });
        // The selected file is kept as long as it is included
        let file = selected
            .and_then(|path| self.changed_files.iter().position(|file| file.path == path))
            .or_else(|| self.first());
        let brief = match touched {
            0 => None,
            1 => Some(Notification::info("1 file changed")),
            n => Some(Notification::info(format!("{n} files changed"))),
        };
        Applied::Done {
            commits,
            changed_files: changed,
            file,
            brief,
        }
    }

    pub fn load(&mut self, file: Option<usize>) -> Show {
        let Some(changed_file) = file.map(|i| self.changed_files[i].clone()) else {
            self.shown = None;
            return Show::Empty {
                pinned: Notification::info("No changed files"),
            };
        };
        let content = self.repo.read(&changed_file);
        self.show(changed_file, content)
    }

    fn show(&mut self, file: ChangedFile, content: Result<[String; 2], String>) -> Show {
        let shown = Some((file.clone(), content.clone()));
        if shown == self.shown {
            return Show::Unchanged;
        }
        let same_file = self
            .shown()
            .is_some_and(|previous| previous.path == file.path);
        self.shown = shown;
        let (diff, pinned) = build_diff(&file, content);
        Show::Diff(Box::new(Shown {
            diff,
            pinned,
            paths: [file.left_side_path().to_owned(), file.path.clone()],
            can_rollback: self.repo.is_local_changes(),
            same_file,
        }))
    }

    /// Run `rolled_back` afterwards, so the list keeps up.
    pub fn rollback(&mut self, request: &RollbackRequest) -> Option<Result<Show, String>> {
        let Some((file, Ok([left_side, _]))) = self.shown.clone() else {
            return None;
        };
        Some(
            self.repo
                .rollback(&file, request.lines.clone(), &request.before)
                .map(|after| self.show(file, Ok([left_side, after]))),
        )
    }

    /// File rollback of `file` in the changed files. Run `rolled_back` afterwards, so the list
    /// keeps up.
    pub fn rollback_file(&self, file: usize) -> Result<(), String> {
        if !self.repo.is_local_changes() {
            return Err(ONLY_LOCAL_CHANGES.into());
        }
        self.repo.rollback_file(&self.changed_files[file])
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
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

        fn read(&self, path: &str) -> Option<String> {
            std::fs::read_to_string(self.0.join(path)).ok()
        }

        /// `git status --short`, with untracked files.
        fn status(&self) -> String {
            let out = Command::new("git")
                .args(["status", "--short", "--untracked-files=all"])
                .current_dir(&self.0)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).into_owned()
        }

        fn repo_source(&self) -> RepoSource {
            let repo = Repo::local_changes(Path::new(&self.0)).unwrap();
            let changed_files = repo.changed_files().unwrap();
            RepoSource::new(repo, changed_files)
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn apply(source: &mut RepoSource, job: ReadJob) -> Applied {
        let read = job.run();
        source.apply(read)
    }

    fn files_changed(source: &mut RepoSource, paths: &[&str]) -> Applied {
        let paths = paths.iter().map(|path| (*path).to_owned()).collect();
        let job = source.files_changed(paths);
        apply(source, job)
    }

    fn brief(applied: Applied) -> Option<Notification> {
        match applied {
            Applied::Done { brief, .. } => brief,
            Applied::Failed { brief, .. } => panic!("read failed: {}", brief.text),
        }
    }

    fn path(source: &RepoSource, file: Option<usize>) -> &str {
        &source.changed_files()[file.unwrap()].path
    }

    fn position(source: &RepoSource, path: &str) -> Option<usize> {
        source
            .changed_files()
            .iter()
            .position(|file| file.path == path)
    }

    #[test]
    fn source_autoreload_keeps_selected_file() {
        let temp_repo = TempRepo::new("keeps");
        temp_repo.write("a.rs", "a\n");
        temp_repo.write("b.rs", "b\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("a.rs", "a2\n");
        temp_repo.write("b.rs", "b2\n");
        let mut source = temp_repo.repo_source();
        let b = position(&source, "b.rs");
        assert!(
            matches!(source.load(b), Show::Diff(shown) if !shown.same_file && shown.can_rollback)
        );
        assert!(matches!(source.load(b), Show::Unchanged));

        temp_repo.write("b.rs", "b3\n");
        temp_repo.write("c.rs", "c\n");
        let Applied::Done {
            changed_files,
            file,
            brief,
            ..
        } = files_changed(&mut source, &["b.rs", "c.rs"])
        else {
            panic!("read failed");
        };
        assert!(changed_files.is_some());
        assert_eq!(path(&source, file), "b.rs");
        assert_eq!(brief, Some(Notification::info("2 files changed")));
        assert!(matches!(source.load(file), Show::Diff(shown) if shown.same_file));
    }

    #[test]
    fn selecting_a_commit_says_nothing() {
        let temp_repo = TempRepo::new("select");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("a.rs", "a2\n");
        let mut source = temp_repo.repo_source();

        let job = source.select_commit(Some("HEAD".into()));
        assert_eq!(brief(apply(&mut source, job)), None);
        temp_repo.write("a.rs", "a3\n");
        let job = source.select_commit(None);
        assert_eq!(brief(apply(&mut source, job)), None);
        // Local changes selected again, after a file is added on disk
        temp_repo.write("b.rs", "b\n");
        let job = source.select_commit(None);
        assert_eq!(brief(apply(&mut source, job)), None);
    }

    #[test]
    fn rolling_back_says_nothing() {
        let temp_repo = TempRepo::new("rollback");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("a.rs", "a2\n");
        temp_repo.write("b.rs", "b\n");
        let mut source = temp_repo.repo_source();
        let a = position(&source, "a.rs");
        assert!(matches!(source.load(a), Show::Diff(_)));

        let request = RollbackRequest {
            lines: [0..1, 0..1],
            before: "a2\n".into(),
        };
        assert!(matches!(source.rollback(&request), Some(Ok(_))));
        let job = source.rolled_back();
        let Applied::Done {
            changed_files,
            file,
            brief,
            ..
        } = apply(&mut source, job)
        else {
            panic!("read failed");
        };
        assert!(changed_files.is_some());
        assert_eq!(path(&source, file), "b.rs");
        assert_eq!(brief, None);
    }

    #[test]
    fn unknown_commit_keeps_the_selected_commit() {
        let temp_repo = TempRepo::new("unknown");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        let mut source = temp_repo.repo_source();
        let job = source.select_commit(Some("missing".into()));
        assert!(matches!(
            apply(&mut source, job),
            Applied::Failed {
                selected_commit: None,
                ..
            }
        ));
        assert!(matches!(
            files_changed(&mut source, &["a.rs"]),
            Applied::Done { file: None, .. }
        ));
        assert!(matches!(source.load(None), Show::Empty { .. }));
    }

    /// Rolls back the file at `path` and reads Local changes again, as the UI does.
    fn file_rollback(source: &mut RepoSource, path: &str) -> Applied {
        let file = position(source, path).unwrap();
        source.rollback_file(file).unwrap();
        let job = source.rolled_back();
        apply(source, job)
    }

    #[test]
    fn file_rollback_of_modified_file_resets_disk_and_index() {
        let temp_repo = TempRepo::new("file-rollback-modified");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("a.rs", "a2\n");
        temp_repo.git(&["add", "a.rs"]);
        temp_repo.write("a.rs", "a3\n");
        let mut source = temp_repo.repo_source();

        let Applied::Done { file, .. } = file_rollback(&mut source, "a.rs") else {
            panic!("read failed");
        };
        assert_eq!(temp_repo.read("a.rs").as_deref(), Some("a\n"));
        assert_eq!(temp_repo.status(), "");
        assert!(source.changed_files().is_empty());
        assert_eq!(file, None);
    }

    #[test]
    fn file_rollback_of_deleted_file_restores_it() {
        let temp_repo = TempRepo::new("file-rollback-deleted");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        std::fs::remove_file(temp_repo.0.join("a.rs")).unwrap();
        let mut source = temp_repo.repo_source();

        file_rollback(&mut source, "a.rs");
        assert_eq!(temp_repo.read("a.rs").as_deref(), Some("a\n"));
        assert_eq!(temp_repo.status(), "");
    }

    #[test]
    fn file_rollback_of_renamed_file_moves_it_back() {
        let temp_repo = TempRepo::new("file-rollback-renamed");
        std::fs::create_dir(temp_repo.0.join("src")).unwrap();
        temp_repo.write("src/a.rs", "a\nb\nc\nd\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        std::fs::create_dir(temp_repo.0.join("lib")).unwrap();
        temp_repo.git(&["mv", "src/a.rs", "lib/b.rs"]);
        temp_repo.write("lib/b.rs", "a\nb\nc\nd\ne\n");
        let mut source = temp_repo.repo_source();

        file_rollback(&mut source, "lib/b.rs");
        assert_eq!(temp_repo.read("src/a.rs").as_deref(), Some("a\nb\nc\nd\n"));
        assert!(!temp_repo.0.join("lib").exists());
        assert_eq!(temp_repo.status(), "");
    }

    #[test]
    fn file_rollback_of_staged_added_file_deletes_it() {
        let temp_repo = TempRepo::new("file-rollback-staged");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("b.rs", "b\n");
        temp_repo.git(&["add", "b.rs"]);
        let mut source = temp_repo.repo_source();

        file_rollback(&mut source, "b.rs");
        assert_eq!(temp_repo.read("b.rs"), None);
        assert_eq!(temp_repo.status(), "");
    }

    #[test]
    fn file_rollback_of_untracked_file_deletes_it() {
        let temp_repo = TempRepo::new("file-rollback-untracked");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        temp_repo.write("a.rs", "a2\n");
        temp_repo.write("b.rs", "b\n");
        let mut source = temp_repo.repo_source();

        let Applied::Done { file, .. } = file_rollback(&mut source, "b.rs") else {
            panic!("read failed");
        };
        assert_eq!(temp_repo.read("b.rs"), None);
        assert_eq!(temp_repo.status(), " M a.rs\n");
        assert_eq!(path(&source, file), "a.rs");
    }

    #[test]
    fn file_rollback_only_in_local_changes() {
        let temp_repo = TempRepo::new("file-rollback-commit");
        temp_repo.write("a.rs", "a\n");
        temp_repo.git(&["add", "-A"]);
        temp_repo.git(&["commit", "-q", "-m", "c"]);
        let mut source = temp_repo.repo_source();
        let job = source.select_commit(Some("HEAD".into()));
        apply(&mut source, job);

        let a = position(&source, "a.rs").unwrap();
        assert_eq!(
            source.rollback_file(a),
            Err("Only Local changes can be rolled back".into())
        );
        assert_eq!(temp_repo.read("a.rs").as_deref(), Some("a\n"));
    }
}
