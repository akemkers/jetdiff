//! Which Split a UI shows: the window with sync scroll, or Aligned Split in the terminal.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitRows {
    /// Each side scrolls on its own, held together by sync scroll.
    Synced,
    /// A line and its counterpart in the same row.
    Aligned,
}
