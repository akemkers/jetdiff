//! Visningsmodell for side-by-side diff av én fil. Ingen GUI-avhengighet, all geometri er
//! piksler som UI-laget tegner rett ut.

pub mod anim;
pub mod columns;
pub mod command;
pub mod cursor;
pub mod divider;
pub mod folding;
pub mod geometry;
pub mod keys;
pub mod layout;
pub mod metrics;
pub mod mode;
pub mod model;
pub mod nav;
pub mod scroll;
mod scrolloff;
pub mod separator;
pub mod state;
pub mod stripe;
pub mod theme;
pub mod unified;

pub use anim::ScrollAnimation;
pub use command::Command;
pub use cursor::Cursor;
pub use divider::{Cubic, DividerBand, DividerSeparator, Point, divider_bands, divider_separators};
pub use folding::{Connector, FoldId, Folding};
pub use geometry::{SideGeometry, UnifiedGeometry, side_geometry};
pub use jetdiff_core::ComparisonPolicy;
pub use jetdiff_core::side::Side;
pub use jetdiff_syntax::{Language, Token};
pub use keys::{Key, KeyCode, KeyParser, Parsed};
pub use layout::{LineLayout, Viewport, VisualFold};
pub use mode::DiffMode;
pub use model::{Block, DiffType, FileDiff, InlineHighlight, LineHighlight};
pub use scroll::{PositionKeeper, ScrollState};
pub use separator::PathSeg;
pub use state::{DiffState, Horizontal, Outcome};
pub use stripe::{StripeMarker, stripe_markers, thumb_range};
pub use theme::{Rgba, SyntaxStyle, Theme};
pub use unified::{Row, UnifiedDiff, UnifiedState};
