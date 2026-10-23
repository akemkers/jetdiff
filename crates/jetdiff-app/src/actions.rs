//! Rollback and `file:line` for the cursor's line.

use std::ops::Range;

use jetdiff_viewmodel::{Cursor, FileDiff, Side};

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
        return Err("Only Local changes can be rolled back");
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

/// `cursor` is the cursor's line in the newest version.
pub fn location(cursor: Cursor, paths: &[String; 2]) -> Option<String> {
    let path = &paths[cursor.side.select(0, 1)];
    (!path.is_empty()).then(|| format!("{path}:{}", cursor.line + 1))
}
