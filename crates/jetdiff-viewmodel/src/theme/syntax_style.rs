//! How a syntax token is drawn: its color, and whether it is bold or italic.

use super::{Rgba, rgb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxStyle {
    pub color: Rgba,
    pub bold: bool,
    pub italic: bool,
}

pub(super) const fn style(hex: u32) -> SyntaxStyle {
    SyntaxStyle {
        color: rgb(hex),
        bold: false,
        italic: false,
    }
}

pub(super) const fn bold(hex: u32) -> SyntaxStyle {
    SyntaxStyle {
        bold: true,
        ..style(hex)
    }
}
