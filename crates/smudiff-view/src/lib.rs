//! Visningsmodell for side-by-side diff av én fil. Ingen GUI-avhengighet, all geometri er
//! piksler som UI-laget tegner rett ut.

pub mod anim;
pub mod divider;
pub mod layout;
pub mod model;
pub mod nav;
pub mod scroll;
pub mod stripe;
pub mod theme;

pub use anim::ScrollAnimation;
pub use divider::{Cubic, DividerBand, Point, divider_bands};
pub use layout::{LineLayout, Viewport};
pub use model::{Block, DiffType, FileDiff, InlineHighlight, LineBackground, LineHighlight};
pub use nav::{NavTarget, Navigation};
pub use scroll::{LineMapping, ScrollState};
pub use smudiff_core::ComparisonPolicy;
pub use smudiff_core::side::Side;
pub use stripe::{StripeMarker, stripe_markers, thumb_range};
pub use theme::{Rgba, Theme};
