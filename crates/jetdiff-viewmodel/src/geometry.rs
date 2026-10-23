//! Horizontal layout for one side. Left is mirrored around the divider, as in IntelliJ:
//! `[text | gutter] [divider] [gutter | text]`, with the scrollbar over the outer edge of the text.
//! Gutter coordinates are local to the Right (unmirrored) side: 0 is the edge toward the divider,
//! `width` is the edge toward the text.

use std::ops::Range;

use jetdiff_core::side::Side;

use crate::divider::DIVIDER_WIDTH;
use crate::model::FileDiff;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GutterLayout {
    pub width: f32,
    /// The line number area, local.
    pub numbers: (f32, f32),
    /// `getWhitespaceSeparatorOffset`: from here out to the text gets the ignored color.
    pub separator: f32,
}

/// Smallest width of the line number area: the breakpoint icon in New UI is 14px
/// (`getInitialLineNumberWidth`).
pub const MIN_NUMBERS_WIDTH: f32 = 14.;

/// `EditorGutterLayout.createNewUILayout` for a read-only diff editor, from the divider outwards:
/// empty annotation area 4, gap 4, line numbers, gap 4, no icons, folding 9 + 2,
/// extra free painters 8 and gap 1. `numbers_width` is the `stringWidth` of the largest line number.
pub fn gutter_layout(numbers_width: f32) -> GutterLayout {
    let numbers = numbers_width.round().max(MIN_NUMBERS_WIDTH);
    let start = 4. + 4.;
    let width = start + numbers + 4. + 11. + 8. + 1.;
    GutterLayout {
        width,
        numbers: (start, start + numbers),
        // `getWhitespaceSeparatorOffset` in New UI
        separator: width - 3.,
    }
}

/// The scrollbar in `EditorMarkupModelImpl`: 14 + thin gap 2 + `minMarkHeight` 2. It is
/// transparent and lies over the text (`editor.transparent.scrollbar`).
pub const SCROLLBAR_THICKNESS: f32 = 18.;
/// Wide hunk minimap marks start at `minMarkHeight + 2` and are 14px.
const HUNK_MINIMAP_X: (f32, f32) = (4., 18.);
/// The Mac thumb is at most 10px, centered in the 14px: `2 + minMark + 14/2 - w/2`.
const THUMB_X: (f32, f32) = (6., 16.);

#[derive(Debug, Clone, PartialEq)]
pub struct SideGeometry {
    pub side: Side,
    /// Left in a pair. A side alone is never mirrored.
    pub mirrored: bool,
    /// The whole text area. The scrollbar lies over the outermost part.
    pub text: Range<f32>,
    /// Where the text starts. Left has the scrollbar on the inside, so the text is moved past it.
    pub text_x: f32,
    pub gutter: Range<f32>,
    pub scrollbar: Range<f32>,
    pub gutter_layout: GutterLayout,
}

impl SideGeometry {
    /// `x` is the left edge of the side and `width` the whole width it gets.
    pub fn new(
        side: Side,
        mirrored: bool,
        x: f32,
        width: f32,
        gutter_layout: GutterLayout,
    ) -> Self {
        let end = x + width;
        let gw = gutter_layout.width.min(width);
        let (text, gutter) = if mirrored {
            (x..end - gw, end - gw..end)
        } else {
            (x + gw..end, x..x + gw)
        };
        let sw = SCROLLBAR_THICKNESS.min(text.end - text.start);
        let scrollbar = if mirrored {
            text.start..text.start + sw
        } else {
            text.end - sw..text.end
        };
        let text_x = if mirrored {
            (text.start + sw).min(text.end)
        } else {
            text.start
        };
        Self {
            side,
            mirrored,
            text,
            text_x,
            gutter,
            scrollbar,
            gutter_layout,
        }
    }

    pub fn bounds(&self) -> Range<f32> {
        if self.mirrored {
            self.text.start..self.gutter.end
        } else {
            self.gutter.start..self.text.end
        }
    }

    /// Local gutter interval to window x. Mirrored on Left.
    pub fn gutter_x(&self, local: Range<f32>) -> Range<f32> {
        if self.mirrored {
            self.gutter.end - local.end..self.gutter.end - local.start
        } else {
            self.gutter.start + local.start..self.gutter.start + local.end
        }
    }

    /// Local scrollbar interval, 0 toward the text, to window x. Mirrored on Left.
    fn scrollbar_x(&self, (start, end): (f32, f32)) -> Range<f32> {
        let clip = |r: Range<f32>| r.start.max(self.scrollbar.start)..r.end.min(self.scrollbar.end);
        if self.mirrored {
            let right = self.scrollbar.start + SCROLLBAR_THICKNESS;
            clip(right - end..right - start)
        } else {
            let left = self.scrollbar.end - SCROLLBAR_THICKNESS;
            clip(left + start..left + end)
        }
    }

    pub fn hunk_minimap_x(&self) -> Range<f32> {
        self.scrollbar_x(HUNK_MINIMAP_X)
    }

    pub fn thumb_x(&self) -> Range<f32> {
        self.scrollbar_x(THUMB_X)
    }

    /// X for the line number. Right is right-aligned in the area. Left is mirrored twice
    /// in IntelliJ, so the glyphs read normally, but left-aligned 1px before the area
    /// (`textOffset = offset - areaWidth - 1`).
    pub fn number_x(&self, text_width: f32) -> f32 {
        let (start, end) = self.gutter_layout.numbers;
        if self.mirrored {
            self.gutter_x(start..end).start - 1.
        } else {
            self.gutter_x(start..end).end - text_width
        }
    }
}

/// Both sides for a window that starts at `x` and is `width` wide.
pub fn side_geometry(
    diff: &FileDiff,
    number_char_width: f32,
    x: f32,
    width: f32,
) -> [SideGeometry; 2] {
    let geom = |side: Side, mirrored, x, width| {
        // The last line number decides the width, like `getMaxLineNumberString`
        let digits = diff.line_count(side).to_string().len();
        let gutter = gutter_layout(digits as f32 * number_char_width);
        SideGeometry::new(side, mirrored, x, width, gutter)
    };
    let side_width = ((width - DIVIDER_WIDTH) / 2.).floor().max(0.);
    [
        geom(Side::Left, true, x, side_width),
        geom(
            Side::Right,
            false,
            x + side_width + DIVIDER_WIDTH,
            width - side_width - DIVIDER_WIDTH,
        ),
    ]
}

/// `GAP_BETWEEN_AREAS` between the number columns for Left and Right in Unified.
const UNIFIED_NUMBER_GAP: f32 = 5.;

/// Unified: one code view without mirroring, with a number column for each side. Both are right-aligned
/// in their column (`EditorGutterComponentImpl.paintLineNumbers`).
#[derive(Debug, Clone, PartialEq)]
pub struct UnifiedGeometry {
    pub code: SideGeometry,
    /// Right edge of the column for Left, local in the gutter.
    left_side_numbers_end: f32,
}

impl UnifiedGeometry {
    /// `calcLineNumberAreaWidth`: the column for Left has the minimum width, the one for Right only its
    /// largest number.
    pub fn new(diff: &FileDiff, number_char_width: f32, x: f32, width: f32) -> Self {
        let numbers_width = |side: Side| {
            let digits = diff.line_count(side).to_string().len();
            (digits as f32 * number_char_width).round()
        };
        let left_side_width = numbers_width(Side::Left).max(MIN_NUMBERS_WIDTH);
        let gutter =
            gutter_layout(left_side_width + UNIFIED_NUMBER_GAP + numbers_width(Side::Right));
        Self {
            code: SideGeometry::new(Side::Right, false, x, width, gutter),
            left_side_numbers_end: gutter.numbers.0 + left_side_width,
        }
    }

    /// X for a line number of `side`, right-aligned in its column.
    pub fn number_x(&self, side: Side, text_width: f32) -> f32 {
        match side {
            Side::Left => self.code.gutter_x(0.0..self.left_side_numbers_end).end - text_width,
            Side::Right => self.code.number_x(text_width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_for_three_digits() {
        // "999" in JetBrains Mono 12 is 21.6px
        let g = gutter_layout(21.6);
        assert_eq!(g.numbers, (8., 30.));
        assert_eq!(g.width, 54.);
        assert_eq!(g.separator, 51.);
        // One line: the breakpoint space is the minimum
        assert_eq!(gutter_layout(7.2).width, 46.);
    }

    const G: GutterLayout = GutterLayout {
        width: 40.,
        numbers: (10., 30.),
        separator: 37.,
    };

    #[test]
    fn left_side_is_mirrored() {
        let g = SideGeometry::new(Side::Left, true, 0., 500., G);
        assert_eq!(g.text, 0. ..460.);
        assert_eq!(g.gutter, 460. ..500.);
        assert_eq!(g.bounds(), 0. ..500.);
        // Outermost 3px toward the text
        assert_eq!(g.gutter_x(G.separator..G.width), 460. ..463.);
        assert_eq!(g.number_x(99.), 469.);
        assert_eq!(g.scrollbar, 0. ..18.);
        assert_eq!(g.hunk_minimap_x(), 0. ..14.);
        assert_eq!(g.thumb_x(), 2. ..12.);
    }

    #[test]
    fn right_side_is_unmirrored() {
        let g = SideGeometry::new(Side::Right, false, 524., 500., G);
        assert_eq!(g.gutter, 524. ..564.);
        assert_eq!(g.text, 564. ..1024.);
        assert_eq!(g.gutter_x(G.separator..G.width), 561. ..564.);
        assert_eq!(g.number_x(16.), 538.);
        assert_eq!(g.scrollbar, 1006. ..1024.);
        assert_eq!(g.hunk_minimap_x(), 1010. ..1024.);
        assert_eq!(g.thumb_x(), 1012. ..1022.);
    }

    #[test]
    fn unified_has_two_number_columns() {
        use crate::ComparisonPolicy;
        let left_side: String = (0..9).map(|i| format!("{i}\n")).collect();
        let right_side: String = (0..120).map(|i| format!("{i}\n")).collect();
        let diff = FileDiff::new(&left_side, &right_side, ComparisonPolicy::Default).unwrap();
        // 10 lines gives two digits, 121 gives three. 7px per digit.
        let g = UnifiedGeometry::new(&diff, 7., 100., 800.);
        assert!(!g.code.mirrored);
        assert_eq!(g.code.bounds(), 100. ..900.);
        // Left column: 8..22, gap 5, Right column: 27..48
        assert_eq!(g.code.gutter_layout.numbers, (8., 48.));
        assert_eq!(g.number_x(Side::Left, 7.), 115.);
        assert_eq!(g.number_x(Side::Right, 7.), 141.);
    }

    #[test]
    fn narrow_window_gives_no_negative_widths() {
        let g = SideGeometry::new(Side::Right, false, 0., 30., G);
        assert_eq!(g.gutter, 0. ..30.);
        assert!(g.text.is_empty());
        assert!(g.scrollbar.is_empty());
        assert!(g.hunk_minimap_x().is_empty());
    }
}
