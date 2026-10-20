//! Tilstanden rundt diffen som UI-ene deler. Se README.

pub mod actions;
pub mod changes;
pub mod commits;
pub mod content;
pub mod files;
pub mod help;
pub mod watcher;

pub use actions::{RollbackRequest, location, rollback_request};
pub use changes::{Changes, Loaded, ReloadJob, Reloaded, Show, Shown};
pub use commits::{Commits, LOCAL_CHANGES, age};
pub use content::{Level, Notice, build_diff, empty_diff};
pub use files::{FileTree, count_label, file_name};
pub use watcher::Watcher;

/// Raden markøren flyttes til, innenfor `len` rader.
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
    fn steg_innenfor_lista() {
        assert_eq!(step(0, true, 3), 1);
        assert_eq!(step(2, true, 3), 2);
        assert_eq!(step(0, false, 3), 0);
        assert_eq!(step(0, true, 0), 0);
    }
}
