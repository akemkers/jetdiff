//! The code in the diff: the visible rows, in Split or Unified, and the scrollbar on the right. Both
//! diff modes are rows from `UnifiedDiff`, and scroll is rounded to whole rows. The side panel and
//! the lines around it are in `side_panel.rs` and `widgets.rs`.

mod hunk_color;
mod painter;
mod scrollbar;
mod split;
mod unified;

use jetdiff_viewmodel::{LineLayout, RowLayout, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use self::painter::Painter;

/// The row on the screen row `screen`, `None` below the last row.
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
