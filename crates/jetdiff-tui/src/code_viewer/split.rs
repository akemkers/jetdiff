//! Left and Right line next to each other, with a separator line between. The side without a line is filled in.

use jetdiff_viewmodel::Side;
use ratatui::layout::Rect;

use super::hunk_color::HunkColor;
use super::painter::Painter;
use crate::geometry;

/// Gives the column the scrollbar is in.
pub(super) fn paint(painter: &mut Painter, area: Rect) -> u16 {
    let cells = geometry::aligned(painter.diff(), area);
    painter.divider(cells.divider, area);
    for visible in painter.visible_rows(area) {
        let (y, row) = (visible.y, visible.row);
        let hunk_color = HunkColor::of_hunk(painter.diff(), row);
        for (columns, side) in cells.sides.iter().zip([Side::Left, Side::Right]) {
            match row.lines[side.select(0, 1)] {
                Some(line) => {
                    painter.line(columns, y, side, line, hunk_color, None);
                    painter.line_number(columns.number_end(), y, line, hunk_color, visible.marked);
                }
                None => painter.filler(columns, y),
            }
            if visible.marked {
                painter.caret(columns, y);
            }
        }
    }
    cells.scrollbar
}
