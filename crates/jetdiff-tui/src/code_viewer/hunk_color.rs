//! The hunk a row belongs to, and the color it should have.

use jetdiff_viewmodel::{FileDiff, HunkKind, Row};

#[derive(Clone, Copy)]
pub(super) struct HunkColor {
    pub(super) kind: HunkKind,
    /// The line gets a fainter background, so the inline highlights stand out.
    pub(super) compared_by_word: bool,
}

impl HunkColor {
    /// The color of the whole hunk, as in Split.
    pub(super) fn of_hunk(diff: &FileDiff, row: &Row) -> Option<Self> {
        row.hunk
            .map(|hunk| Self::new(diff, hunk, diff.hunks()[hunk].kind))
    }

    /// The color of the row, deleted or added, as in Unified.
    pub(super) fn of_row(diff: &FileDiff, row: &Row) -> Option<Self> {
        row.hunk.map(|hunk| Self::new(diff, hunk, row.kind()))
    }

    fn new(diff: &FileDiff, hunk: usize, kind: HunkKind) -> Self {
        Self {
            kind,
            compared_by_word: diff.hunks()[hunk].compared_by_word(),
        }
    }
}
