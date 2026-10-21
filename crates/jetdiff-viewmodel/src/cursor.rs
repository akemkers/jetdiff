//! The cursor for the whole diff, on the side that has focus. That side is the master for sync
//! scroll, like `getCurrentSide` in `TwosideTextDiffViewer`.

use jetdiff_core::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub side: Side,
    /// Logical line. On a collapsed fold it is the first line of the fold.
    pub line: usize,
}

impl Cursor {
    pub fn new(side: Side, line: usize) -> Self {
        Self { side, line }
    }
}
