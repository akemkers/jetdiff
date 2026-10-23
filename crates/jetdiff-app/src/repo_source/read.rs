//! Reading a repo source again in the background: why, the job and what it read.

use std::path::PathBuf;

use jetdiff_files::ChangedFile;
use jetdiff_git::{Commit, Repo};

/// Why a repo source is read again.
pub(super) enum Trigger {
    /// The paths the watcher saw change.
    FilesChanged(Vec<String>),
    CommitSelected,
    RolledBack,
}

pub struct RepoRead {
    pub(super) trigger: Trigger,
    pub(super) commit: Option<String>,
    pub(super) repo: Repo,
    pub(super) changed_files: Vec<ChangedFile>,
    pub(super) commits: Vec<Commit>,
}

pub struct ReadJob {
    pub(super) root: PathBuf,
    pub(super) commit: Option<String>,
    pub(super) trigger: Trigger,
}

impl ReadJob {
    /// The commit list is fetched again every time, since Local changes decide which commits it has.
    pub fn run(self) -> Result<RepoRead, String> {
        let local_changes = Repo::local_changes(&self.root)?;
        let commits = local_changes.commits()?;
        let repo = match &self.commit {
            Some(hash) => Repo::commit(&self.root, hash)?,
            None => local_changes,
        };
        let changed_files = repo.changed_files()?;
        Ok(RepoRead {
            trigger: self.trigger,
            commit: self.commit,
            repo,
            changed_files,
            commits,
        })
    }
}
