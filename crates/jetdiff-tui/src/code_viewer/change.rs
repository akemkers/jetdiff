//! Endringen en rad hører til, og fargen den skal ha.

use jetdiff_viewmodel::{DiffType, FileDiff, Row};

#[derive(Clone, Copy)]
pub(super) struct Change {
    pub(super) diff_type: DiffType,
    /// Blokken har ord-markering, så linja får svakere bakgrunn og ordene står ut. Navnet er
    /// fra IntelliJ.
    pub(super) ignored: bool,
}

impl Change {
    /// Fargen til hele blokken, som i split.
    pub(super) fn of_block(diff: &FileDiff, row: &Row) -> Option<Self> {
        row.block
            .map(|block| Self::new(diff, block, diff.blocks()[block].diff_type))
    }

    /// Fargen til raden, slettet eller ny, som i unified.
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
