//! What the tree and the commit list have in common: sizes, keys and how the selection and the
//! cursor look.

use gpui::{Div, KeyDownEvent, Stateful, prelude::*};

use crate::theme::UiPalette;

pub const ROW_HEIGHT: f32 = 22.;
pub const TEXT_SIZE: f32 = 15.;
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

/// The selection is filled, whatever else the row shows. The cursor, shown only with focus, is an
/// outline in the accent color. On the same row both show. Text in the row that has a color of its
/// own takes the strong color while selected.
pub fn selection_and_cursor(
    row: Stateful<Div>,
    selected: bool,
    cursor: bool,
    focused: bool,
    palette: &UiPalette,
) -> Stateful<Div> {
    row.border_1()
        .border_color(if cursor && focused {
            palette.accent
        } else {
            gpui::transparent_black()
        })
        .when(selected, |row| {
            row.bg(palette.selected).text_color(palette.strong)
        })
}
