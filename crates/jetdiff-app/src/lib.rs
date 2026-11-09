//! The state around the diff that the UIs share. See the README.

pub mod actions;
pub mod commits;
pub mod content;
pub mod file_pair;
pub mod files;
pub mod help;
pub mod keys;
pub mod notification;
pub mod notification_slot;
pub mod overlay_slot;
pub mod repo_source;
pub mod walkthrough;
pub mod watcher;

pub use actions::{RollbackRequest, cursor_location, rollback_request};
pub use commits::{CommitRow, Commits, LOCAL_CHANGES, Selected, age};
pub use content::{build_diff, empty_diff};
pub use file_pair::FilePair;
pub use files::{Files, count_label, file_name};
pub use keys::{AppCommand, BINDINGS, app_command};
pub use notification::{Notification, NotificationLevel};
pub use notification_slot::{NotificationSlot, Progress, SlotContent};
pub use overlay_slot::{Overlay, OverlaySlot};
pub use repo_source::{Applied, ReadJob, RepoRead, RepoSource, Show, Shown};
pub use watcher::{DiskChange, Watcher};

/// The row the cursor moves to, within `len` rows.
pub fn step(row: usize, down: bool, len: usize) -> usize {
    if down {
        (row + 1).min(len.saturating_sub(1))
    } else {
        row.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_within_the_list() {
        assert_eq!(step(0, true, 3), 1);
        assert_eq!(step(2, true, 3), 2);
        assert_eq!(step(0, false, 3), 0);
        assert_eq!(step(0, true, 0), 0);
    }
}
