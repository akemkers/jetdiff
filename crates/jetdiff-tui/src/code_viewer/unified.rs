//! One line per row, with old and new line number in the gutter. The words get the row's color.

use ratatui::layout::Rect;

use super::hunk_color::HunkColor;
use super::painter::Painter;
use crate::geometry;

/// Gives the column the scrollbar is in.
pub(super) fn paint(painter: &mut Painter, area: Rect) -> u16 {
    let cells = geometry::unified(painter.diff(), area);
    let columns = &cells.columns;
    let number_ends = [cells.old_end, columns.number_end()];
    for visible in painter.visible_rows(area) {
        let (y, row) = (visible.y, visible.row);
        let hunk_color = HunkColor::of_row(painter.diff(), row);
        let inline_highlight_kind = Some(row.kind());
        painter.line(
            columns,
            y,
            row.side(),
            row.line(),
            hunk_color,
            inline_highlight_kind,
        );
        for (line, end) in row.lines.into_iter().zip(number_ends) {
            if let Some(line) = line {
                painter.line_number(end, y, line, hunk_color, visible.marked);
            }
        }
        if visible.marked {
            painter.cursor_row(columns, y);
        }
    }
    cells.scrollbar
}
