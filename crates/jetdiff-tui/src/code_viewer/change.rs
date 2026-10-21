//! The hunk a row belongs to, and the color it should have.

use jetdiff_viewmodel::{DiffType, FileDiff, Row};

#[derive(Clone, Copy)]
pub(super) struct Change {
    pub(super) diff_type: DiffType,
    /// The hunk has inline highlights, so the line gets a fainter background and the words stand out. The name is
    /// from IntelliJ.
    pub(super) ignored: bool,
}

impl Change {
    /// The color of the whole hunk, as in Split.
    pub(super) fn of_block(diff: &FileDiff, row: &Row) -> Option<Self> {
        row.block
            .map(|block| Self::new(diff, block, diff.blocks()[block].diff_type))
    }

    /// The color of the row, deleted or added, as in Unified.
    pub(super) fn of_row(diff: &FileDiff, row: &Row) -> Option<Self> {
        row.block
            .map(|block| Self::new(diff, block, row.diff_type()))
    }

    fn new(diff: &FileDiff, block: usize, diff_type: DiffType) -> Self {
        Self {
            diff_type,
            ignored: diff.blocks()[block].ignored(),
        }
    }
}
