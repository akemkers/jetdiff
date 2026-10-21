//! The state around the diff that the UIs share. See the README.

pub mod actions;
pub mod changes;
pub mod commits;
pub mod content;
pub mod files;
pub mod help;
pub mod keys;
pub mod watcher;

pub use actions::{RollbackRequest, location, rollback_request};
pub use changes::{Changes, Loaded, ReloadJob, Reloaded, Show, Shown};
pub use commits::{Commits, LOCAL_CHANGES, age};
pub use content::{Level, Notice, build_diff, empty_diff};
pub use files::{FileTree, count_label, file_name};
pub use keys::{AppCommand, BINDINGS, app_command};
pub use watcher::Watcher;

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
