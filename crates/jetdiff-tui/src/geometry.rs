//! Sidene i celler. Venstre side er speilet som i GPUI, så numrene står inn mot divideren.

use std::ops::Range;

use jetdiff_viewmodel::{FileDiff, Side};
use ratatui::layout::Rect;

pub const DIVIDER_WIDTH: u16 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideCells {
    pub side: Side,
    pub mirrored: bool,
    pub gutter: Range<u16>,
    pub text: Range<u16>,
    pub scrollbar: u16,
    pub digits: u16,
}

impl SideCells {
    fn new(side: Side, mirrored: bool, x: u16, width: u16, digits: u16) -> Self {
        let end = x + width;
        let gutter_width = (digits + 2).min(width);
        let rest = width - gutter_width;
        if mirrored {
            Self {
                side,
                mirrored,
                gutter: end - gutter_width..end,
                text: (x + 1).min(x + rest)..x + rest,
                scrollbar: x,
                digits,
            }
        } else {
            Self {
                side,
                mirrored,
                gutter: x..x + gutter_width,
                text: x + gutter_width..end.saturating_sub(1).max(x + gutter_width),
                scrollbar: end.saturating_sub(1),
                digits,
            }
        }
    }

    pub fn contains(&self, x: u16) -> bool {
        let start = self.gutter.start.min(self.scrollbar);
        let end = self.gutter.end.max(self.scrollbar + 1);
        (start..end).contains(&x)
    }

    pub fn text_width(&self) -> u16 {
        self.text.end - self.text.start
    }

    pub fn number(&self, line: usize) -> (u16, String) {
        let text = (line + 1).to_string();
        let x = if self.mirrored {
            self.gutter.start + 1
        } else {
            (self.gutter.end - 1).saturating_sub(text.len() as u16)
        };
        (x, text)
    }
}

fn digits(diff: &FileDiff, side: Side) -> u16 {
    diff.line_count(side).to_string().len() as u16
}

pub fn split(diff: &FileDiff, area: Rect) -> [SideCells; 2] {
    let side_width = area.width.saturating_sub(DIVIDER_WIDTH) / 2;
    let right_x = area.x + side_width + DIVIDER_WIDTH.min(area.width);
    let right_width = area.right().saturating_sub(right_x);
    [
        SideCells::new(
            Side::Left,
            true,
            area.x,
            side_width,
            digits(diff, Side::Left),
        ),
        SideCells::new(
            Side::Right,
            false,
            right_x,
            right_width,
            digits(diff, Side::Right),
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedCells {
    pub editor: SideCells,
    pub old_digits: u16,
}

impl UnifiedCells {
    pub fn numbers(&self, lines: [Option<usize>; 2]) -> [Option<(u16, String)>; 2] {
        let start = self.editor.gutter.start;
        let old_end = start + 1 + self.old_digits;
        let new_end = self.editor.gutter.end - 1;
        let at = |line: usize, end: u16| {
            let text = (line + 1).to_string();
            (end.saturating_sub(text.len() as u16), text)
        };
        [
            lines[0].map(|l| at(l, old_end)),
            lines[1].map(|l| at(l, new_end)),
        ]
    }
}

pub fn unified(diff: &FileDiff, area: Rect) -> UnifiedCells {
    let old = digits(diff, Side::Left);
    let new = digits(diff, Side::Right);
    UnifiedCells {
        editor: SideCells::new(Side::Right, false, area.x, area.width, old + 1 + new),
        old_digits: old,
    }
}

#[cfg(test)]
mod tests {
    use jetdiff_viewmodel::ComparisonPolicy;

    use super::*;

    #[test]
    fn sidene_deler_bredden() {
        let text = "x\n".repeat(120);
        let diff = FileDiff::new(&text, &text, ComparisonPolicy::Default).unwrap();
        let [l, r] = split(&diff, Rect::new(0, 0, 44, 10));
        // 20 per side og 4 i divideren. Tre siffer gir gutter på 5.
        assert_eq!(
            (l.scrollbar, l.text.clone(), l.gutter.clone()),
            (0, 1..15, 15..20)
        );
        assert_eq!(
            (r.gutter.clone(), r.text.clone(), r.scrollbar),
            (24..29, 29..43, 43)
        );
        assert_eq!(l.number(8), (16, "9".into()));
        assert_eq!(r.number(8), (27, "9".into()));
        assert_eq!(r.number(119), (25, "120".into()));
    }

    #[test]
    fn unified_har_to_nummerkolonner() {
        let diff = FileDiff::new("a\n", &"b\n".repeat(10), ComparisonPolicy::Default).unwrap();
        let u = unified(&diff, Rect::new(0, 0, 40, 10));
        // " 1 10 " før teksten
        assert_eq!(u.editor.gutter, 0..6);
        assert_eq!(
            u.numbers([Some(0), Some(9)]),
            [Some((1, "1".into())), Some((3, "10".into()))]
        );
    }
}
