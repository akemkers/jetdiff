//! The sides in cells. One scrollbar at the far right, since the rows are shared.

use std::ops::Range;

use jetdiff_viewmodel::{FileDiff, Side};
use ratatui::layout::Rect;

pub struct Columns {
    pub gutter: Range<u16>,
    pub text: Range<u16>,
}

impl Columns {
    fn new(x: u16, width: u16, numbers: u16) -> Self {
        let gutter = (numbers + 2).min(width);
        Self {
            gutter: x..x + gutter,
            text: x + gutter..x + width,
        }
    }

    pub fn row(&self, y: u16) -> Rect {
        Rect::new(self.gutter.start, y, self.text.end - self.gutter.start, 1)
    }

    pub fn gutter(&self, y: u16) -> Rect {
        Rect::new(self.gutter.start, y, self.gutter.end - self.gutter.start, 1)
    }

    pub fn text(&self, y: u16) -> Rect {
        Rect::new(self.text.start, y, self.text.end - self.text.start, 1)
    }

    pub fn text_width(&self) -> u16 {
        self.text.end - self.text.start
    }

    pub fn number_end(&self) -> u16 {
        self.gutter.end - 1
    }
}

/// The line number right-aligned to `end`.
pub fn number(line: usize, end: u16) -> (u16, String) {
    let text = (line + 1).to_string();
    (end.saturating_sub(text.len() as u16), text)
}

fn digits(diff: &FileDiff, side: Side) -> u16 {
    diff.line_count(side).to_string().len() as u16
}

fn without_scrollbar(area: Rect) -> (Rect, u16) {
    let width = area.width.saturating_sub(1);
    (Rect { width, ..area }, area.x + width)
}

pub struct Aligned {
    pub sides: [Columns; 2],
    pub divider: u16,
    pub scrollbar: u16,
}

pub fn aligned(diff: &FileDiff, area: Rect) -> Aligned {
    let (area, scrollbar) = without_scrollbar(area);
    let left = area.width.saturating_sub(1) / 2;
    let divider = area.x + left;
    let right_x = (divider + 1).min(area.right());
    Aligned {
        sides: [
            Columns::new(area.x, left, digits(diff, Side::Left)),
            Columns::new(right_x, area.right() - right_x, digits(diff, Side::Right)),
        ],
        divider,
        scrollbar,
    }
}

/// Old and new number in the gutter.
pub struct Unified {
    pub columns: Columns,
    pub old_end: u16,
    pub scrollbar: u16,
}

pub fn unified(diff: &FileDiff, area: Rect) -> Unified {
    let (area, scrollbar) = without_scrollbar(area);
    let old = digits(diff, Side::Left);
    let columns = Columns::new(area.x, area.width, old + 1 + digits(diff, Side::Right));
    Unified {
        old_end: columns.gutter.start + 1 + old,
        columns,
        scrollbar,
    }
}

#[cfg(test)]
mod tests {
    use jetdiff_viewmodel::ComparisonPolicy;

    use super::*;

    #[test]
    fn the_sides_share_the_width() {
        let text = "x\n".repeat(120);
        let diff = FileDiff::new(&text, &text, ComparisonPolicy::Default).unwrap();
        let a = aligned(&diff, Rect::new(0, 0, 44, 10));
        // 21 per side, divider and scrollbar. Three digits gives a gutter of 5.
        assert_eq!(
            (a.sides[0].gutter.clone(), a.sides[0].text.clone()),
            (0..5, 5..21)
        );
        assert_eq!(a.divider, 21);
        assert_eq!(
            (a.sides[1].gutter.clone(), a.sides[1].text.clone()),
            (22..27, 27..43)
        );
        assert_eq!(a.scrollbar, 43);
        let right = &a.sides[1];
        assert_eq!(number(8, right.number_end()), (25, "9".into()));
        assert_eq!(number(119, right.number_end()), (23, "120".into()));
    }

    #[test]
    fn unified_has_two_number_columns() {
        let diff = FileDiff::new("a\n", &"b\n".repeat(10), ComparisonPolicy::Default).unwrap();
        let u = unified(&diff, Rect::new(0, 0, 40, 10));
        // " 1 10 " before the text
        assert_eq!(u.columns.gutter, 0..6);
        assert_eq!(number(0, u.old_end), (1, "1".into()));
        assert_eq!(number(9, u.columns.number_end()), (3, "10".into()));
    }
}
