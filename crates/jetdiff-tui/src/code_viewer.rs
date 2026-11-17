//! The code in the diff: the visible rows, in Split or Unified, and the scrollbar on the right. Both
//! diff modes are rows from `UnifiedDiff`, and scroll is rounded to whole rows. The side panel and
//! the lines around it are in `side_panel.rs` and `widgets.rs`.

mod hunk_color;
mod painter;
mod scrollbar;
mod split;
mod unified;

use jetdiff_viewmodel::{LineLayout, RowLayout, SplitShare, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use self::painter::Painter;

/// The row on the screen row `screen`, `None` below the last row.
pub fn row_at(layout: &LineLayout, scroll_y: f32, screen: u16) -> Option<usize> {
    let row = layout.y_to_logical_line(scroll_y.round() + screen as f32);
    (row < layout.line_count()).then_some(row)
}

pub fn paint(
    state: &UnifiedState,
    split_share: SplitShare,
    focused: bool,
    area: Rect,
    theme: &Theme,
    buf: &mut Buffer,
) {
    let mut painter = Painter::new(state, theme, focused, buf);
    painter.background(area);
    let scrollbar = match state.diff.row_layout() {
        RowLayout::AlignedSplit => split::paint(&mut painter, area, split_share),
        RowLayout::Unified => unified::paint(&mut painter, area),
    };
    scrollbar::paint(&mut painter, scrollbar, area);
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, UnifiedDiff};

    use super::*;

    #[test]
    fn a_closed_fold_is_a_fold_wave_without_line_numbers() {
        let left_side: String = (0..30).map(|i| format!("a{i}\n")).collect();
        let right_side = left_side.replace("a20\n", "x\n");
        let diff = FileDiff::new(&left_side, &right_side, ComparisonPolicy::Default).unwrap();
        let diff = Rc::new(diff);
        let area = Rect::new(0, 0, 40, 10);
        for row_layout in [RowLayout::Unified, RowLayout::AlignedSplit] {
            let unified_diff = UnifiedDiff::with_layout(diff.clone(), row_layout);
            let state = UnifiedState::new(Rc::new(unified_diff), true);
            let mut buf = Buffer::empty(area);
            paint(
                &state,
                SplitShare::default(),
                true,
                area,
                &Theme::built_in(),
                &mut buf,
            );
            let screen_row =
                |y: u16| -> String { (0..area.width - 1).map(|x| buf[(x, y)].symbol()).collect() };
            // Lines 0..16 are the first fold, and line 16 comes after it
            let fold = screen_row(0);
            assert!(fold.contains('~'), "{fold}");
            assert!(!fold.chars().any(|c| c.is_ascii_digit()), "{fold}");
            assert!(screen_row(1).contains("a16"));
        }
    }
}
