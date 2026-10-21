//! Vertical geometry for one side. Everything that goes from line to y goes here, so scroll, divider
//! and stripe see folding only through visual lines. A collapsed fold is one visual
//! line of normal height, like `FoldRegion` with a placeholder in `EditorImpl`.

use std::ops::Range;

/// `editor.virtual.lines`: the diff editors can be scrolled five lines past the end.
const ADDITIONAL_LINES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub height: f32,
    pub line_height: f32,
}

/// Collapsed fold: logical lines `lines` are shown as the single visual line `visual`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualFold {
    pub lines: Range<usize>,
    pub visual: usize,
}

impl VisualFold {
    /// Lines hidden up to and including this fold.
    fn hidden_through(&self) -> usize {
        self.lines.end - self.visual - 1
    }
}

/// Folds must be sorted, disjoint and at least two lines, as `FoldingModelSupport` makes them.
pub fn visual_folds(ranges: impl IntoIterator<Item = Range<usize>>) -> Vec<VisualFold> {
    let mut hidden = 0;
    let mut out: Vec<VisualFold> = Vec::new();
    for lines in ranges {
        debug_assert!(lines.len() >= 2);
        debug_assert!(out.last().is_none_or(|f| f.lines.end <= lines.start));
        let visual = lines.start - hidden;
        hidden += lines.len() - 1;
        out.push(VisualFold { lines, visual });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineLayout<'a> {
    line_count: usize,
    line_height: f32,
    folds: &'a [VisualFold],
}

impl LineLayout<'static> {
    pub fn new(line_count: usize, line_height: f32) -> Self {
        LineLayout::with_folds(line_count, line_height, &[])
    }
}

impl<'a> LineLayout<'a> {
    pub fn with_folds(line_count: usize, line_height: f32, folds: &'a [VisualFold]) -> Self {
        assert!(line_height > 0.);
        Self {
            line_count: line_count.max(1),
            line_height,
            folds,
        }
    }

    pub fn folds(&self) -> &'a [VisualFold] {
        self.folds
    }

    /// The collapsed fold shown on the visual line.
    pub fn fold_at_visual(&self, visual: usize) -> Option<&'a VisualFold> {
        let i = self.folds.partition_point(|f| f.visual < visual);
        self.folds.get(i).filter(|f| f.visual == visual)
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    pub fn line_count(&self) -> usize {
        self.line_count
    }

    pub fn visual_line_count(&self) -> usize {
        self.line_count - self.folds.last().map_or(0, VisualFold::hidden_through)
    }

    /// Lines inside a fold give the fold's line. Past the end it maps linearly, like
    /// `logicalToVisualPosition` in virtual space.
    pub fn logical_to_visual(&self, line: usize) -> usize {
        let i = self.folds.partition_point(|f| f.lines.start <= line);
        match i.checked_sub(1).map(|i| &self.folds[i]) {
            None => line,
            Some(f) if line < f.lines.end => f.visual,
            Some(f) => line - f.hidden_through(),
        }
    }

    /// A fold gives its first line, like `visualToLogicalPosition` for column 0.
    pub fn visual_to_logical(&self, visual: usize) -> usize {
        let i = self.folds.partition_point(|f| f.visual <= visual);
        match i.checked_sub(1).map(|i| &self.folds[i]) {
            None => visual,
            Some(f) if f.visual == visual => f.lines.start,
            Some(f) => visual + f.hidden_through(),
        }
    }

    pub fn visual_line_to_y(&self, visual: usize) -> f32 {
        visual as f32 * self.line_height
    }

    /// `EditorCoordinateMapper.yToVisualLine`: everything above the first line is line 0, and past
    /// the end it continues linearly.
    pub fn y_to_visual_line(&self, y: f32) -> usize {
        if y < self.line_height {
            return 0;
        }
        (y / self.line_height).floor() as usize
    }

    pub fn y_to_logical_line(&self, y: f32) -> usize {
        self.visual_to_logical(self.y_to_visual_line(y))
    }

    pub fn line_top(&self, line: usize) -> f32 {
        self.visual_line_to_y(self.logical_to_visual(line))
    }

    /// Exclusive bottom of the line.
    pub fn line_bottom(&self, line: usize) -> f32 {
        self.line_top(line) + self.line_height
    }

    /// `DiffDrawUtil.getGutterMarkerPaintRange`. An empty range gives `y1 == y2` on the line boundary,
    /// and line 0 is moved 1px down so the 2px band becomes `[0, 2)`.
    pub fn marker_range(&self, lines: Range<usize>) -> (f32, f32) {
        if lines.is_empty() {
            let y = if lines.start == 0 {
                self.line_top(0) + 1.
            } else {
                self.line_bottom(lines.start - 1)
            };
            (y, y)
        } else {
            (self.line_top(lines.start), self.line_bottom(lines.end - 1))
        }
    }

    /// The y interval a hunk covers in the code view. An empty side becomes 2px `[boundary-1, boundary+1)`,
    /// the same band that `DiffLineMarkerRenderer` and `DiffDefaultBorderRenderer` draw.
    pub fn block_band(&self, lines: Range<usize>) -> (f32, f32) {
        let (y1, y2) = self.marker_range(lines);
        if y2 - y1 <= 2. {
            (y1 - 1., y1 + 1.)
        } else {
            (y1, y2)
        }
    }

    /// `EditorSizeManager.getPreferredHeight` with `additionalLinesCount`.
    pub fn content_height(&self) -> f32 {
        (self.visual_line_count() + ADDITIONAL_LINES) as f32 * self.line_height
    }

    pub fn max_scroll(&self, viewport_height: f32) -> f32 {
        (self.content_height() - viewport_height).max(0.)
    }

    pub fn clamp_scroll(&self, y: f32, viewport_height: f32) -> f32 {
        y.clamp(0., self.max_scroll(viewport_height))
    }

    /// Logical lines that are fully or partly visible, a closed interval like
    /// `DiffDividerDrawUtil.getPaintedInterval`.
    pub fn painted_lines(&self, scroll_y: f32, viewport_height: f32) -> (usize, usize) {
        (
            self.y_to_logical_line(scroll_y),
            self.y_to_logical_line(scroll_y + viewport_height),
        )
    }

    /// The same interval in visual lines, what the code view actually draws.
    pub fn painted_visual_lines(&self, scroll_y: f32, viewport_height: f32) -> (usize, usize) {
        (
            self.y_to_visual_line(scroll_y),
            self.y_to_visual_line(scroll_y + viewport_height),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_to_line() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.y_to_visual_line(-5.), 0);
        assert_eq!(l.y_to_visual_line(19.9), 0);
        assert_eq!(l.y_to_visual_line(20.), 1);
        assert_eq!(l.y_to_visual_line(250.), 12);
    }

    #[test]
    fn marker_range_for_empty_and_non_empty() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.marker_range(0..0), (1., 1.));
        assert_eq!(l.marker_range(3..3), (60., 60.));
        assert_eq!(l.marker_range(2..4), (40., 80.));
        // After the last line, e.g. a missing '\n' at the end
        assert_eq!(l.marker_range(10..10), (200., 200.));
    }

    #[test]
    fn block_band_gives_2px_for_empty_side() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.block_band(0..0), (0., 2.));
        assert_eq!(l.block_band(3..3), (59., 61.));
        assert_eq!(l.block_band(3..5), (60., 100.));
    }

    #[test]
    fn scroll_is_clamped_with_five_extra_lines() {
        let l = LineLayout::new(100, 20.);
        assert_eq!(l.content_height(), 2100.);
        assert_eq!(l.max_scroll(600.), 1500.);
        assert_eq!(l.clamp_scroll(-3., 600.), 0.);
        assert_eq!(l.clamp_scroll(9999., 600.), 1500.);
        assert_eq!(LineLayout::new(3, 20.).max_scroll(600.), 0.);
    }

    /// 20 lines, 3..7 and 10..15 collapsed.
    fn folded() -> Vec<VisualFold> {
        visual_folds([3..7, 10..15])
    }

    #[test]
    fn fold_gives_one_visual_line() {
        let folds = folded();
        assert_eq!(folds[0].visual, 3);
        assert_eq!(folds[1].visual, 7);
        let l = LineLayout::with_folds(20, 10., &folds);
        assert_eq!(l.visual_line_count(), 20 - 3 - 4);
        let expected = [
            0, 1, 2, 3, 3, 3, 3, 4, 5, 6, 7, 7, 7, 7, 7, 8, 9, 10, 11, 12,
        ];
        for (line, &v) in expected.iter().enumerate() {
            assert_eq!(l.logical_to_visual(line), v, "line {line}");
        }
        // Past the end, linearly
        assert_eq!(l.logical_to_visual(25), 18);
        let back: Vec<_> = (0..14).map(|v| l.visual_to_logical(v)).collect();
        assert_eq!(back, [0, 1, 2, 3, 7, 8, 9, 10, 15, 16, 17, 18, 19, 20]);
    }

    #[test]
    fn visual_and_logical_are_consistent() {
        let folds = visual_folds([0..4, 5..7, 9..12, 18..20]);
        let l = LineLayout::with_folds(20, 10., &folds);
        for v in 0..l.visual_line_count() + 10 {
            assert_eq!(
                l.logical_to_visual(l.visual_to_logical(v)),
                v,
                "visual {v}"
            );
        }
        for line in 0..30 {
            let v = l.logical_to_visual(line);
            let start = l.visual_to_logical(v);
            assert!(start <= line);
            match l.fold_at_visual(v) {
                Some(f) => assert!(f.lines.contains(&line)),
                None => assert_eq!(start, line),
            }
        }
    }

    #[test]
    fn y_with_folds() {
        let folds = folded();
        let l = LineLayout::with_folds(20, 10., &folds);
        assert_eq!(l.line_top(5), 30.);
        assert_eq!(l.line_bottom(6), 40.);
        assert_eq!(l.line_top(7), 40.);
        assert_eq!(l.y_to_logical_line(35.), 3);
        assert_eq!(l.y_to_logical_line(45.), 7);
        assert_eq!(l.content_height(), (13 + 5) as f32 * 10.);
        assert_eq!(l.painted_lines(30., 50.), (3, 15));
        assert_eq!(l.painted_visual_lines(30., 50.), (3, 8));
        assert_eq!(l.fold_at_visual(7).map(|f| f.lines.clone()), Some(10..15));
        assert_eq!(l.fold_at_visual(6), None);
    }
}
