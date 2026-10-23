//! What a repo source tells the UI to show, after a read and for a file.

use std::rc::Rc;

use jetdiff_files::ChangedFile;
use jetdiff_git::Commit;
use jetdiff_viewmodel::FileDiff;

use crate::notification::Notification;

pub enum Applied {
    Failed {
        selected_commit: Option<String>,
        brief: Notification,
    },
    Done {
        commits: Vec<Commit>,
        /// Only when the list has changed.
        changed_files: Option<Rc<[ChangedFile]>>,
        file: Option<usize>,
        /// Only after files changed on disk.
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
    /// Only Local changes can be written to, not a commit.
    pub can_rollback: bool,
    pub same_file: bool,
}
