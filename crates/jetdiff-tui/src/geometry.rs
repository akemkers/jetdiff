//! The sides in cells. One scrollbar at the far right, since the rows are shared.

use std::ops::Range;

use jetdiff_viewmodel::{FileDiff, Side, SplitShare, split_text_width};
use ratatui::layout::Rect;

const DIFF_DIVIDER_WIDTH: u16 = 1;

pub struct Columns {
    pub gutter: Range<u16>,
    pub text: Range<u16>,
    /// The gutter has room for the line numbers.
    numbers_fit: bool,
}

impl Columns {
    fn new(x: u16, width: u16, numbers: u16) -> Self {
        let full_gutter = numbers + 2;
        let gutter = full_gutter.min(width);
        Self {
            gutter: x..x + gutter,
            text: x + gutter..x + width,
            numbers_fit: width >= full_gutter,
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

    /// `None` when the side is too narrow for the line numbers.
    pub fn number_end(&self) -> Option<u16> {
        self.numbers_fit.then(|| self.gutter.end - 1)
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

impl Aligned {
    /// The text width horizontal scroll uses, see `split_text_width`.
    pub fn text_width(&self) -> u16 {
        split_text_width([&self.sides[0], &self.sides[1]].map(Columns::text_width))
    }
}

/// Left gets `split_share` of the width, rounded to whole columns.
pub fn aligned(diff: &FileDiff, area: Rect, split_share: SplitShare) -> Aligned {
    let (area, scrollbar) = without_scrollbar(area);
    let sides_width = area.width.saturating_sub(DIFF_DIVIDER_WIDTH);
    let left_side_width =
        (split_share.left_side_width(sides_width as f32).round() as u16).min(sides_width);
    let divider = area.x + left_side_width;
    let right_side_x = (divider + DIFF_DIVIDER_WIDTH).min(area.right());
    Aligned {
        sides: [
            Columns::new(area.x, left_side_width, digits(diff, Side::Left)),
            Columns::new(
                right_side_x,
                area.right() - right_side_x,
                digits(diff, Side::Right),
            ),
        ],
        divider,
        scrollbar,
    }
}

/// The share that puts the diff divider in column `x`.
pub fn split_share_at(area: Rect, x: u16) -> SplitShare {
    let (area, _) = without_scrollbar(area);
    let middle = x.saturating_sub(area.x) as f32 + 0.5;
    SplitShare::at(middle, area.width as f32, DIFF_DIVIDER_WIDTH as f32)
}

/// A number column for each side in the gutter.
pub struct Unified {
    pub columns: Columns,
    left_side_number_end: u16,
    pub scrollbar: u16,
}

impl Unified {
    /// Where the line numbers of `side` end. `None` when the gutter is too narrow for them.
    pub fn number_end(&self, side: Side) -> Option<u16> {
        let right_side_end = self.columns.number_end()?;
        Some(side.select(self.left_side_number_end, right_side_end))
    }
}

pub fn unified(diff: &FileDiff, area: Rect) -> Unified {
    let (area, scrollbar) = without_scrollbar(area);
    let left_side_digits = digits(diff, Side::Left);
    let columns = Columns::new(
        area.x,
        area.width,
        left_side_digits + 1 + digits(diff, Side::Right),
    );
    Unified {
        left_side_number_end: columns.gutter.start + 1 + left_side_digits,
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
        let a = aligned(&diff, Rect::new(0, 0, 44, 10), SplitShare::default());
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
        let right_side = &a.sides[1];
        let number_end = right_side.number_end().unwrap();
        assert_eq!(number(8, number_end), (25, "9".into()));
        assert_eq!(number(119, number_end), (23, "120".into()));
    }

    #[test]
    fn the_share_follows_the_dragged_column() {
        let text = "x\n".repeat(120);
        let diff = FileDiff::new(&text, &text, ComparisonPolicy::Default).unwrap();
        let area = Rect::new(10, 0, 44, 10);
        let at = |x| aligned(&diff, area, split_share_at(area, x));
        let a = at(20);
        assert_eq!(a.divider, 20);
        assert_eq!((a.sides[0].gutter.start, a.sides[0].text.end), (10, 20));
        assert_eq!((a.sides[1].gutter.start, a.sides[1].text.end), (21, 53));
        // Narrower side
        assert_eq!(a.text_width(), 5);
        assert_eq!(at(45).divider, 45);
    }

    #[test]
    fn a_side_can_get_zero_columns() {
        let text = "x\n".repeat(120);
        let diff = FileDiff::new(&text, &text, ComparisonPolicy::Default).unwrap();
        let area = Rect::new(10, 0, 44, 10);
        // Share 0: Left is empty and Right gets all 42 columns
        let left_side_empty = aligned(&diff, area, split_share_at(area, 0));
        assert_eq!(left_side_empty.divider, 10);
        assert_eq!(left_side_empty.sides[0].row(0).width, 0);
        assert_eq!(left_side_empty.sides[0].number_end(), None);
        assert_eq!(left_side_empty.sides[1].row(0), Rect::new(11, 0, 42, 1));
        assert_eq!(left_side_empty.text_width(), 37);
        // Share 1: the other way around
        let right_side_empty = aligned(&diff, area, split_share_at(area, 60));
        assert_eq!(right_side_empty.divider, 52);
        assert_eq!(right_side_empty.sides[0].row(0), Rect::new(10, 0, 42, 1));
        assert_eq!(right_side_empty.sides[1].row(0).width, 0);
        assert_eq!(right_side_empty.scrollbar, 53);
    }

    #[test]
    fn unified_has_two_number_columns() {
        let diff = FileDiff::new("a\n", &"b\n".repeat(10), ComparisonPolicy::Default).unwrap();
        let u = unified(&diff, Rect::new(0, 0, 40, 10));
        // " 1 10 " before the text
        assert_eq!(u.columns.gutter, 0..6);
        assert_eq!(
            number(0, u.number_end(Side::Left).unwrap()),
            (1, "1".into())
        );
        assert_eq!(
            number(9, u.number_end(Side::Right).unwrap()),
            (3, "10".into())
        );
    }
}
