//! The colors of the walkthrough view, from its design. Dark, and every text color at least 4.5:1
//! against the background it is on.

use gpui::{Hsla, rgb};

pub(super) const PAGE: u32 = 0x16181f;
/// The contents and the cards.
pub(super) const SURFACE: u32 = 0x1c1f28;
pub(super) const BORDER: u32 = 0x2a2e3b;
pub(super) const SELECTED: u32 = 0x2b3042;
pub(super) const CODE_BACKGROUND: u32 = 0x262a36;
/// Lighter than the code, so each file starts with a bar of its own.
pub(super) const FILE_HEADER: u32 = 0x262a36;

/// Headings and bold text.
pub(super) const STRONG: u32 = 0xffffff;
pub(super) const PRIMARY: u32 = 0xe6e8ef;
pub(super) const BODY: u32 = 0xd4d7e1;
pub(super) const SUBTITLE: u32 = 0xc9cdd9;
pub(super) const MUTED: u32 = 0x9aa0b4;
pub(super) const STEP_NUMBER: u32 = 0x7d849a;

pub(super) const ACCENT: u32 = 0x9cc4ff;
pub(super) const BUTTON: u32 = 0x6fa8ff;
pub(super) const BUTTON_TEXT: u32 = 0x0f1420;
pub(super) const READ: u32 = 0x7fd4a0;
pub(super) const KEY_DOT: u32 = 0x6fa8ff;

/// The note tags differ in lightness, not only in hue: background and text.
pub(super) const DECISION: (u32, u32) = (0x3a3016, 0xf3cf75);
pub(super) const DIVERGENCE: (u32, u32) = (0x1d3050, 0x9cc4ff);
pub(super) const LIMITATION: (u32, u32) = (0x3f2818, 0xf5ad80);
pub(super) const UNTESTED: (u32, u32) = (0x3e1f2a, 0xf5a3b6);
/// Upkeep has no color of its own in the design, so it gets the quiet one.
pub(super) const UPKEEP: (u32, u32) = (0x262a36, 0xd4d7e1);

pub(super) fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}
