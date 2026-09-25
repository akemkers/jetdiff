//! Det treet og commit-lista har felles: mål, taster og radfarger.

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
    Open,
}

pub fn list_key(ev: &KeyDownEvent) -> Option<ListKey> {
    if ev.keystroke.modifiers.modified() {
        return None;
    }
    match ev.keystroke.key.as_str() {
        "j" | "down" => Some(ListKey::Down),
        "k" | "up" => Some(ListKey::Up),
        "enter" | "l" => Some(ListKey::Open),
        _ => None,
    }
}

/// Raden markøren flyttes til, innenfor `len` rader.
pub fn step(row: usize, down: bool, len: usize) -> usize {
    if down {
        (row + 1).min(len.saturating_sub(1))
    } else {
        row.saturating_sub(1)
    }
}

/// Markøren vises bare med fokus. Det som er åpnet er svakere.
pub fn row_bg(theme: &UiTheme, cursor: bool, open: bool, focused: bool) -> Option<Hsla> {
    if focused && cursor {
        Some(theme.tree_selection)
    } else if open {
        Some(theme.tree_selection_inactive)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steg_innenfor_lista() {
        assert_eq!(step(0, true, 3), 1);
        assert_eq!(step(2, true, 3), 2);
        assert_eq!(step(0, false, 3), 0);
        assert_eq!(step(0, true, 0), 0);
    }
}
