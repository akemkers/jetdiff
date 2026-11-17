//! What the mouse pointer is over in the diff, found by the UI that knows where things are drawn.

use jetdiff_core::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerTarget {
    /// A line on one side, in Split with sync scroll.
    Line(Side, usize),
    /// A row, in Unified and Aligned Split.
    Row(usize),
}
