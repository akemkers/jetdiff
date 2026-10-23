//! What the tree and the commit list have in common: sizes, keys and row colors.

use gpui::{Hsla, KeyDownEvent};

use crate::theme::UiTheme;

pub const ROW_HEIGHT: f32 = 22.;
pub const TEXT_SIZE: f32 = 14.;
pub const PAD_LEFT: f32 = 8.;
pub const PAD_RIGHT: f32 = 12.;
pub const GAP: f32 = 4.;

pub enum ListKey {
    Down,
    Up,
    Select,
}

pub fn list_key(ev: &KeyDownEvent) -> Option<ListKey> {
    if ev.keystroke.modifiers.modified() {
        return None;
    }
    match ev.keystroke.key.as_str() {
        "j" | "down" => Some(ListKey::Down),
        "k" | "up" => Some(ListKey::Up),
        "enter" | "l" => Some(ListKey::Select),
        _ => None,
    }
}

/// The cursor is shown only with focus. The selection is fainter.
pub fn row_bg(theme: &UiTheme, cursor: bool, selected: bool, focused: bool) -> Option<Hsla> {
    if focused && cursor {
        Some(theme.tree_selection)
    } else if selected {
        Some(theme.tree_selection_inactive)
    } else {
        None
    }
}
