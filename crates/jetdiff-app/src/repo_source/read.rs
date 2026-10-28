//! Reading a repo source again in the background: why, the job and what it read.

use std::path::PathBuf;

use jetdiff_files::ChangedFile;
use jetdiff_git::{Commit, Repo};

use crate::commits::Selected;

/// Why a repo source is read again.
pub(super) enum Trigger {
    /// The paths the watcher saw change.
    FilesChanged(Vec<String>),
    /// A commit, amend, reset or rebase.
    HeadMoved,
    SelectionChanged,
    RolledBack,
}

pub struct RepoRead {
    pub(super) trigger: Trigger,
    /// What was read. Not always what was asked for, if its row is gone.
    pub(super) selected: Selected,
    pub(super) repo: Repo,
    pub(super) changed_files: Vec<ChangedFile>,
    pub(super) branch_name: Option<String>,
    pub(super) commits: Vec<Commit>,
}

pub struct ReadJob {
    pub(super) root: PathBuf,
    pub(super) selected: Selected,
    pub(super) trigger: Trigger,
}

impl ReadJob {
    /// The commit list is read again every time, since a commit or rebase changes it.
    pub fn run(self) -> Result<RepoRead, String> {
        let branch = Repo::branch(&self.root)?;
        let commits = branch.commits()?;
        let branch_name = branch.branch_name();
        let selected = self.selected.within(&commits);
        let repo = match &selected {
            Selected::Branch => branch,
            Selected::LocalChanges => Repo::local_changes(&self.root)?,
            Selected::Commit(hash) => Repo::commit(&self.root, hash)?,
        };
        let changed_files = repo.changed_files()?;
        Ok(RepoRead {
            trigger: self.trigger,
            selected,
            repo,
            changed_files,
            branch_name,
            commits,
        })
    }
}
