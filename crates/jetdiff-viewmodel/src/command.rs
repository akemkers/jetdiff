//! Everything the user can ask the diff to do. Keys are translated to this in `keys`, and `DiffState` carries it out.

use jetdiff_core::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    MoveDown,
    MoveUp,
    ScrollLeft,
    ScrollRight,
    /// All the way to the left.
    ScrollToStart,
    /// The end of the cursor's line to the right edge.
    ScrollToEnd,
    GoToTop,
    GoToBottom,
    HalfPageDown,
    HalfPageUp,
    NextHunk,
    PreviousHunk,
    /// Does nothing when only one side is shown, or the side is already active.
    FocusSide(Side),
    /// The fold at the cursor, like `ExpandRegionAction`.
    FoldOpen,
    /// Opens all if any are closed, otherwise closes all.
    ToggleFolds,
    /// The cursor location to the clipboard.
    CopyCursorLocation,
    /// Roll back the hunk on the cursor's line to Left, in the file on disk.
    Rollback,
    ZoomIn,
    ZoomOut,
    ResetZoom,
}
