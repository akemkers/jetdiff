//! Rollback, file rollback and the cursor location.

use std::ops::Range;

use jetdiff_viewmodel::{Cursor, FileDiff, Side};

use crate::files::Files;

/// Both rollback and file rollback write to disk, which only Local changes have.
pub(crate) const ONLY_LOCAL_CHANGES: &str = "Only Local changes can be rolled back";

pub const ROLLBACK_QUESTION: &str = "Roll back this hunk?";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRequest {
    /// Right as it is shown, to detect whether the file has changed since.
    pub before: String,
    pub lines: [Range<usize>; 2],
}

/// `hunk` is the hunk the cursor is in.
pub fn rollback_request(
    diff: &FileDiff,
    hunk: Option<usize>,
    can_rollback: bool,
) -> Result<RollbackRequest, &'static str> {
    if !can_rollback {
        return Err(ONLY_LOCAL_CHANGES);
    }
    if diff.single_side().is_some() {
        return Err("The whole file is added or deleted");
    }
    let hunk = hunk.ok_or("No hunk on this line")?;
    Ok(RollbackRequest {
        before: diff.text(Side::Right).to_owned(),
        lines: diff.hunks()[hunk].lines.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRollbackRequest {
    /// An index in the changed files.
    pub file: usize,
    path: String,
}

impl FileRollbackRequest {
    pub fn question(&self) -> String {
        format!("Roll back {}?", self.path)
    }
}

/// The file on the cursor's row in the file tree. A folder can't be rolled back.
pub fn file_rollback_request(
    files: &Files,
    can_rollback: bool,
) -> Result<FileRollbackRequest, &'static str> {
    if !can_rollback {
        return Err(ONLY_LOCAL_CHANGES);
    }
    let file = files
        .file_at_cursor()
        .ok_or("Select a file to roll it back")?;
    Ok(FileRollbackRequest {
        file,
        path: files.changed_files()[file].path.clone(),
    })
}

/// `cursor` is the cursor's line in the newest version.
pub fn cursor_location(cursor: Cursor, paths: &[String; 2]) -> Option<String> {
    let path = &paths[cursor.side.select(0, 1)];
    (!path.is_empty()).then(|| format!("{path}:{}", cursor.line + 1))
}
