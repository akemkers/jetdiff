//! Venstre og høyre linje side om side, med skillelinje mellom. Siden uten linje fylles ut.

use jetdiff_viewmodel::Side;
use ratatui::layout::Rect;

use super::change::Change;
use super::painter::Painter;
use crate::geometry;

/// Gir kolonnen scrollbaren står i.
pub(super) fn paint(painter: &mut Painter, area: Rect) -> u16 {
    let cells = geometry::aligned(painter.diff(), area);
    painter.divider(cells.divider, area);
    for visible in painter.visible_rows(area) {
        let (y, row) = (visible.y, visible.row);
        let change = Change::of_block(painter.diff(), row);
        for (columns, side) in cells.sides.iter().zip([Side::Left, Side::Right]) {
            match row.lines[side.select(0, 1)] {
                Some(line) => {
                    painter.line(columns, y, side, line, change, None);
                    painter.line_number(columns.number_end(), y, line, change, visible.marked);
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
