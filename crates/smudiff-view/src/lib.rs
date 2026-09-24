//! Visningsmodell for side-by-side diff av én fil. Ingen GUI-avhengighet, all geometri er
//! piksler som UI-laget tegner rett ut.

pub mod anim;
pub mod columns;
pub mod command;
pub mod cursor;
pub mod divider;
pub mod folding;
pub mod keys;
pub mod layout;
pub mod model;
pub mod nav;
pub mod scroll;
pub mod separator;
pub mod state;
pub mod stripe;
pub mod theme;

pub use anim::ScrollAnimation;
pub use command::Command;
pub use cursor::Cursor;
pub use divider::{Cubic, DividerBand, DividerSeparator, Point, divider_bands, divider_separators};
pub use folding::{Connector, FoldId, FoldSettings, Folding};
pub use keys::{Key, KeyCode, KeyParser, Parsed};
pub use layout::{LineLayout, Viewport, VisualFold};
pub use model::{Block, DiffType, FileDiff, InlineHighlight, LineBackground, LineHighlight};
pub use nav::{NavTarget, Navigation};
pub use scroll::{LineMapping, PositionKeeper, ScrollState};
pub use separator::PathSeg;
pub use smudiff_core::ComparisonPolicy;
pub use smudiff_core::side::Side;
pub use state::{DiffState, Horizontal, Outcome};
pub use stripe::{StripeMarker, stripe_markers, thumb_range};
pub use theme::{Rgba, Theme};
