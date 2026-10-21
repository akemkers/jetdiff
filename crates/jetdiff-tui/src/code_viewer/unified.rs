//! Én linje per rad, med gammelt og nytt linjenummer i gutteren. Ordene får radens farge.

use ratatui::layout::Rect;

use super::change::Change;
use super::painter::Painter;
use crate::geometry;

/// Gir kolonnen scrollbaren står i.
pub(super) fn paint(painter: &mut Painter, area: Rect) -> u16 {
    let cells = geometry::unified(painter.diff(), area);
    let columns = &cells.columns;
    let number_ends = [cells.old_end, columns.number_end()];
    for visible in painter.visible_rows(area) {
        let (y, row) = (visible.y, visible.row);
        let change = Change::of_row(painter.diff(), row);
        let word_type = Some(row.diff_type());
        painter.line(columns, y, row.side(), row.line(), change, word_type);
        for (line, end) in row.lines.into_iter().zip(number_ends) {
            if let Some(line) = line {
                painter.line_number(end, y, line, change, visible.marked);
            }
        }
        if visible.marked {
            painter.caret(columns, y);
        }
    }
    cells.scrollbar
}
