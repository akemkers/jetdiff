use std::path::PathBuf;
use std::process::Command;

use super::*;
use crate::commits::CommitRow;

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("jetdiff-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "t@t"]);
        repo.git(&["config", "user.name", "t"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
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
        String::from_utf8_lossy(&out.stdout).trim_end().to_owned()
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }

    /// The hash of the new commit.
    fn commit(&self) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c"]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn open(&self) -> (RepoSource, Commits) {
        RepoSource::open(&self.0).unwrap()
    }

    /// `a.rs` on main, `b.rs` committed on `feature`, and `a.rs` changed but not committed.
    fn feature_branch(name: &str) -> Self {
        let temp_repo = Self::new(name);
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        temp_repo.git(&["checkout", "-q", "-b", "feature"]);
        temp_repo.write("b.rs", "b\n");
        temp_repo.commit();
        temp_repo.write("a.rs", "a2\n");
        temp_repo
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
    let job = source.changed_on_disk(DiskChange::Files(paths));
    apply(source, job)
}

fn select(source: &mut RepoSource, selected: Selected) -> Applied {
    let job = source.select(selected);
    apply(source, job)
}

fn brief(applied: Applied) -> Option<Notification> {
    match applied {
        Applied::Done { brief, .. } => brief,
        Applied::Failed { brief, .. } => panic!("read failed: {}", brief.text),
    }
}

fn selected(applied: &Applied) -> &Selected {
    match applied {
        Applied::Done { selected, .. } | Applied::Failed { selected, .. } => selected,
    }
}

fn path(source: &RepoSource, file: Option<usize>) -> &str {
    &source.changed_files()[file.unwrap()].path
}

fn paths(source: &RepoSource) -> Vec<&str> {
    let mut paths: Vec<_> = source
        .changed_files()
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    paths.sort();
    paths
}

fn position(source: &RepoSource, path: &str) -> Option<usize> {
    source
        .changed_files()
        .iter()
        .position(|file| file.path == path)
}

fn can_rollback(source: &mut RepoSource, path: &str) -> bool {
    match source.load(position(source, path)) {
        Show::Diff(shown) => shown.can_rollback,
        _ => panic!("{path} is not shown"),
    }
}

#[test]
fn starts_on_branch_with_committed_and_uncommitted() {
    let temp_repo = TempRepo::feature_branch("branch");
    temp_repo.write("new.rs", "new\n");
    let (mut source, commits) = temp_repo.open();
    assert!(matches!(commits.row(0), CommitRow::Branch("feature")));
    assert!(commits.is_selected(0));
    assert_eq!(paths(&source), ["a.rs", "b.rs", "new.rs"]);
    assert!(!can_rollback(&mut source, "a.rs"));
    let request = RollbackRequest {
        lines: [0..1, 0..1],
        before: "a2\n".into(),
    };
    assert!(matches!(source.rollback(&request), Some(Err(_))));

    select(&mut source, Selected::LocalChanges);
    assert_eq!(paths(&source), ["a.rs", "new.rs"]);
    assert!(can_rollback(&mut source, "a.rs"));
}

#[test]
fn starts_on_local_changes_without_own_commits() {
    let temp_repo = TempRepo::new("main");
    temp_repo.write("a.rs", "a\n");
    temp_repo.commit();
    temp_repo.write("a.rs", "a2\n");
    let (mut source, commits) = temp_repo.open();
    // Only the Local changes row, so no commits from main
    assert_eq!(commits.len(), 1);
    assert!(matches!(commits.row(0), CommitRow::LocalChanges));
    assert!(can_rollback(&mut source, "a.rs"));
}

#[test]
fn a_commit_that_is_gone_gives_branch() {
    let temp_repo = TempRepo::feature_branch("gone");
    let (mut source, _) = temp_repo.open();
    let hash = temp_repo.git(&["rev-parse", "HEAD"]);
    let applied = select(&mut source, Selected::Commit(hash));
    assert!(matches!(selected(&applied), Selected::Commit(_)));
    assert_eq!(paths(&source), ["b.rs"]);

    temp_repo.git(&["commit", "-q", "--amend", "-m", "amended"]);
    let job = source.changed_on_disk(DiskChange::HeadMoved);
    let applied = apply(&mut source, job);
    assert_eq!(selected(&applied), &Selected::Branch);
    assert_eq!(paths(&source), ["a.rs", "b.rs"]);
}

#[test]
fn a_commit_keeps_the_shown_file() {
    let temp_repo = TempRepo::feature_branch("committed");
    let (mut source, _) = temp_repo.open();
    let a = position(&source, "a.rs");
    assert!(matches!(source.load(a), Show::Diff(_)));

    temp_repo.commit();
    let job = source.changed_on_disk(DiskChange::HeadMoved);
    let Applied::Done {
        commits,
        changed_files,
        file,
        brief,
        ..
    } = apply(&mut source, job)
    else {
        panic!("read failed");
    };
    assert_eq!(commits.len(), 2);
    assert!(changed_files.is_none());
    assert_eq!(brief, None);
    assert!(matches!(source.load(file), Show::Unchanged));
}

#[test]
fn source_autoreload_keeps_selected_file() {
    let temp_repo = TempRepo::new("keeps");
    temp_repo.write("a.rs", "a\n");
    temp_repo.write("b.rs", "b\n");
    temp_repo.commit();
    temp_repo.write("a.rs", "a2\n");
    temp_repo.write("b.rs", "b2\n");
    let (mut source, _) = temp_repo.open();
    let b = position(&source, "b.rs");
    assert!(matches!(source.load(b), Show::Diff(shown) if !shown.same_file && shown.can_rollback));
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
fn selecting_says_nothing() {
    let temp_repo = TempRepo::feature_branch("select");
    let (mut source, _) = temp_repo.open();
    let hash = temp_repo.git(&["rev-parse", "HEAD"]);

    assert_eq!(brief(select(&mut source, Selected::Commit(hash))), None);
    temp_repo.write("a.rs", "a3\n");
    assert_eq!(brief(select(&mut source, Selected::LocalChanges)), None);
    // Local changes selected again, after a file is added on disk
    temp_repo.write("c.rs", "c\n");
    assert_eq!(brief(select(&mut source, Selected::LocalChanges)), None);
}

#[test]
fn rolling_back_says_nothing() {
    let temp_repo = TempRepo::new("rollback");
    temp_repo.write("a.rs", "a\n");
    temp_repo.commit();
    temp_repo.write("a.rs", "a2\n");
    temp_repo.write("b.rs", "b\n");
    let (mut source, _) = temp_repo.open();
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
fn a_failed_read_keeps_the_selection() {
    let temp_repo = TempRepo::feature_branch("failed");
    let (mut source, _) = temp_repo.open();
    let hash = temp_repo.git(&["rev-parse", "HEAD"]);
    select(&mut source, Selected::LocalChanges);
    std::fs::remove_dir_all(temp_repo.0.join(".git")).unwrap();
    let applied = select(&mut source, Selected::Commit(hash));
    assert!(matches!(applied, Applied::Failed { .. }));
    assert_eq!(selected(&applied), &Selected::LocalChanges);
}
