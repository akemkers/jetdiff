//! View model for the diff of one file, Split or Unified. No GUI dependency; all geometry is
//! pixels that the UI layer draws as is.

pub mod anim;
pub mod columns;
pub mod command;
pub mod cursor;
pub mod divider;
pub mod fold_paths;
pub mod folding;
pub mod geometry;
pub mod hunk_minimap;
pub mod keys;
pub mod layout;
pub mod metrics;
pub mod mode;
pub mod model;
pub mod nav;
pub mod scroll;
mod scrolloff;
pub mod state;
pub mod theme;
pub mod unified;

pub use anim::ScrollAnimation;
pub use command::Command;
pub use cursor::Cursor;
pub use divider::{Cubic, DividerBand, FoldLink, Point, divider_bands, fold_links};
pub use fold_paths::PathSeg;
pub use folding::{ClosedFold, FoldId, Folding, UnifiedFolding};
pub use geometry::{SideGeometry, UnifiedGeometry, side_geometry};
pub use hunk_minimap::{HunkMinimapMark, hunk_minimap_marks, thumb_range};
pub use jetdiff_core::ComparisonPolicy;
pub use jetdiff_core::side::Side;
pub use jetdiff_syntax::{Language, Token};
pub use keys::{Key, KeyCode, KeyParser, Parsed};
pub use layout::{LineLayout, Viewport, VisualFold};
pub use mode::DiffMode;
pub use model::{FileDiff, Hunk, HunkKind, InlineHighlight, LineHighlight};
pub use scroll::{PositionKeeper, ScrollState};
pub use state::{DiffState, Horizontal, Outcome};
pub use theme::{Rgba, SyntaxStyle, Theme};
pub use unified::{Row, RowLayout, UnifiedDiff, UnifiedState};
