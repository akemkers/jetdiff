//! Koden i diffen: radene som synes, i split eller unified, og scrollbaren til høyre. Begge
//! visningene er rader fra `UnifiedDiff`, og scroll rundes til hele rader. Panelet og linjene
//! rundt ligger i `panel.rs` og `widgets.rs`.

mod change;
mod painter;
mod scrollbar;
mod split;
mod unified;

use jetdiff_viewmodel::{LineLayout, RowLayout, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use self::painter::Painter;

/// Raden på skjermraden `screen`, `None` under siste rad.
pub fn row_at(layout: &LineLayout, scroll_y: f32, screen: u16) -> Option<usize> {
    let row = layout.y_to_logical_line(scroll_y.round() + screen as f32);
    (row < layout.line_count()).then_some(row)
}

pub fn paint(state: &UnifiedState, focused: bool, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let mut painter = Painter::new(state, theme, focused, buf);
    painter.background(area);
    let scrollbar = match state.diff.row_layout() {
        RowLayout::Split => split::paint(&mut painter, area),
        RowLayout::Unified => unified::paint(&mut painter, area),
    };
    scrollbar::paint(&mut painter, scrollbar, area);
}
