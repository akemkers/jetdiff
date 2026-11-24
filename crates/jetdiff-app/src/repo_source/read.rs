//! Reading a repo source again in the background: why, the job and what it read.

use std::path::{Path, PathBuf};

use jetdiff_files::ChangedFile;
use jetdiff_git::{History, Repo};

use crate::commits::Selected;

/// What jetdiff was started on, which decides what a read gives.
#[derive(Clone)]
pub(super) enum Opened {
    /// Branch, with the commit list to pick a slice of it.
    Branch,
    /// As resolved at startup, so a source reload gives the same commit, also for a branch name or
    /// `HEAD~1`.
    Commit(Repo),
    PullRequest,
}

/// Why a repo source is read again.
pub(super) enum Trigger {
    /// The paths the watcher saw change.
    FilesChanged(Vec<String>),
    /// A commit, amend, reset or rebase.
    HeadMoved,
    SelectionChanged,
    RolledBack,
    SourceReload,
}

pub struct RepoRead {
    pub(super) trigger: Trigger,
    /// What was read. Not always what was asked for, if its row is gone.
    pub(super) selected: Selected,
    pub(super) repo: Repo,
    pub(super) changed_files: Vec<ChangedFile>,
    pub(super) branch_name: Option<String>,
    pub(super) history: History,
}

impl RepoRead {
    fn new(
        trigger: Trigger,
        selected: Selected,
        repo: Repo,
        branch_name: Option<String>,
        history: History,
    ) -> Result<Self, String> {
        let changed_files = repo.changed_files()?;
        Ok(Self {
            trigger,
            selected,
            repo,
            changed_files,
            branch_name,
            history,
        })
    }
}

pub struct ReadJob {
    pub(super) root: PathBuf,
    pub(super) opened: Opened,
    pub(super) selected: Selected,
    pub(super) trigger: Trigger,
}

impl ReadJob {
    pub fn run(self) -> Result<RepoRead, String> {
        let Self {
            root,
            opened,
            selected,
            trigger,
        } = self;
        match opened {
            Opened::Branch => read_branch(&root, selected, trigger),
            Opened::Commit(repo) => {
                RepoRead::new(trigger, selected, repo, None, History::default())
            }
            Opened::PullRequest => Err("A pull request can't be reloaded yet".into()),
        }
    }
}

/// The commit list is read again every time, since a commit or rebase changes it.
fn read_branch(root: &Path, selected: Selected, trigger: Trigger) -> Result<RepoRead, String> {
    let branch = Repo::branch(root)?;
    let history = branch.commits()?;
    let branch_name = branch.branch_name();
    let selected = selected.within(&history);
    let repo = match &selected {
        Selected::Branch => branch,
        Selected::LocalChanges => Repo::local_changes(root)?,
        Selected::Commit(hash) => Repo::commit(root, hash)?,
    };
    RepoRead::new(trigger, selected, repo, branch_name, history)
}
