//! What a repo source tells the UI to show, after a read and for a file.

use std::rc::Rc;

use jetdiff_files::ChangedFile;
use jetdiff_git::Commit;
use jetdiff_viewmodel::FileDiff;

use crate::commits::Selected;
use crate::notification::Notification;

pub enum Applied {
    Failed {
        /// Still what is shown.
        selected: Selected,
        brief: Notification,
    },
    Done {
        /// For the commit list, which shows what was read.
        branch_name: Option<String>,
        commits: Vec<Commit>,
        selected: Selected,
        /// Only when the list has changed.
        changed_files: Option<Rc<[ChangedFile]>>,
        file: Option<usize>,
        /// After files changed on disk, not after HEAD moved, and after a source reload.
        brief: Option<Notification>,
    },
}

pub enum Show {
    Empty { pinned: Notification },
    Unchanged,
    Diff(Box<Shown>),
}

pub struct Shown {
    pub diff: FileDiff,
    pub pinned: Option<Notification>,
    pub paths: [String; 2],
    /// Only in Local changes, so rollback always goes back to HEAD.
    pub can_rollback: bool,
    pub same_file: bool,
}
