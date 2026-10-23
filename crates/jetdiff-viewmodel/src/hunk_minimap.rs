//! The hunk minimap in the scrollbar. The diff highlighters have `ERROR_STRIPE_COLOR` over
//! `DiffUtil.getLinesRange(start, end, false)`, and the position is computed as in
//! `EditorMarkupModelImpl.offsetsToYPositions`.

use jetdiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::LineLayout;
use crate::model::{FileDiff, HunkKind};

/// `DaemonCodeAnalyzerSettings.myErrorStripeMarkMinHeight`.
const MIN_MARK_HEIGHT: f32 = 2.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HunkMinimapMark {
    pub kind: HunkKind,
    /// Y in the code view, pixels from the top of the document.
    pub start_y: f32,
    pub end_y: f32,
}

impl HunkMinimapMark {
    /// Pixels in the track. If the document is shorter than the track it maps 1:1, otherwise it is scaled
    /// and cut to integers. At least 2px high.
    pub fn track_range(&self, content_height: f32, track_height: f32) -> (f32, f32) {
        let map = |y: f32| {
            if content_height < track_height {
                y
            } else {
                (y / content_height * track_height).trunc()
            }
        };
        let start = map(self.start_y);
        let end = map(self.end_y).max(start);
        (start, end.max(start + MIN_MARK_HEIGHT))
    }
}

/// The thumb in the track, `None` when all the content fits. Proportional like a Swing scrollbar
/// with `content_height` as the max and the viewport as the extent.
pub fn thumb_range(
    scroll_y: f32,
    viewport_height: f32,
    content_height: f32,
    track_height: f32,
) -> Option<(f32, f32)> {
    if content_height <= viewport_height {
        return None;
    }
    let scale = track_height / content_height;
    Some((
        scroll_y * scale,
        ((scroll_y + viewport_height) * scale).min(track_height),
    ))
}

/// With folds: y is visual, like `offsetToY` in `EditorMarkupModelImpl`.
pub fn hunk_minimap_marks(
    diff: &FileDiff,
    folding: &Folding,
    side: Side,
    line_height: f32,
) -> Vec<HunkMinimapMark> {
    let layout = folding.layout(side, line_height);
    diff.hunks()
        .iter()
        .map(|hunk| {
            let (start_y, end_y) = hunk_minimap_mark_y(&layout, hunk.lines(side));
            HunkMinimapMark {
                kind: hunk.kind,
                start_y,
                end_y,
            }
        })
        .collect()
}

/// The end offset is without the last line break, so the end line is `end - 1` and the mark ends
/// at its top. An empty range lies at `start`, clamped to the last line.
pub(crate) fn hunk_minimap_mark_y(
    layout: &LineLayout,
    lines: std::ops::Range<usize>,
) -> (f32, f32) {
    let last = layout.line_count() - 1;
    let start = lines.start.min(last);
    let end = if lines.is_empty() {
        start
    } else {
        lines.end - 1
    };
    let start_y = layout.line_top(start);
    // `startLineNumber == endLineNumber` gives `endY = startY`
    let end_y = if end == start {
        start_y
    } else {
        layout.line_top(end)
    };
    (start_y, end_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    #[test]
    fn marks_with_folds_are_visual() {
        let a: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let b = a.replace("a50\n", "x\n");
        let diff = diff(&a, &b);
        let folding = Folding::new(&diff);
        let marks = hunk_minimap_marks(&diff, &folding, Side::Left, 10.);
        // 0..46 folds to one line, so line 50 is visual line 5
        assert_eq!(folding.layout(Side::Left, 10.).logical_to_visual(50), 5);
        assert_eq!(marks[0].start_y, 50.);
        assert_eq!(marks.len(), 1);
    }

    #[test]
    fn marks_per_side() {
        // Left lines 1..2 deleted, Right has an empty side at line 1
        let diff = diff("a\nb\nc\nd\ne\n", "a\nc\nx\ny\nz\n");
        let left_side = hunk_minimap_marks(&diff, &Folding::none(&diff), Side::Left, 10.);
        let right_side = hunk_minimap_marks(&diff, &Folding::none(&diff), Side::Right, 10.);
        assert_eq!(diff.hunks()[0].lines, [1..2, 1..1]);
        assert_eq!((left_side[0].start_y, left_side[0].end_y), (10., 10.));
        assert_eq!((right_side[0].start_y, right_side[0].end_y), (10., 10.));
        assert_eq!(diff.hunks()[1].lines, [3..5, 2..5]);
        // Right 2..5: top of line 2 to top of line 4
        assert_eq!((right_side[1].start_y, right_side[1].end_y), (20., 40.));
        assert_eq!(right_side[1].kind, HunkKind::Modified);
    }

    #[test]
    fn empty_range_after_last_line_is_clamped() {
        // Left is missing '\n' at the end; the hunk is on line 3 of 3
        let diff = diff("one\ntwo\nthree", "one\ntwo\nthree\n");
        assert_eq!(diff.hunks()[0].lines, [3..3, 3..4]);
        let left_side = hunk_minimap_marks(&diff, &Folding::none(&diff), Side::Left, 10.);
        assert_eq!((left_side[0].start_y, left_side[0].end_y), (20., 20.));
    }

    #[test]
    fn thumb_is_proportional() {
        assert_eq!(thumb_range(0., 500., 400., 500.), None);
        assert_eq!(thumb_range(0., 250., 1000., 500.), Some((0., 125.)));
        assert_eq!(thumb_range(750., 250., 1000., 500.), Some((375., 500.)));
    }

    #[test]
    fn track_scales_and_has_minimum_height() {
        let mark = HunkMinimapMark {
            kind: HunkKind::Added,
            start_y: 150.,
            end_y: 150.,
        };
        assert_eq!(mark.track_range(300., 600.), (150., 152.));
        assert_eq!(mark.track_range(1000., 333.), (49., 51.));
        let mark = HunkMinimapMark {
            end_y: 400.,
            ..mark
        };
        assert_eq!(mark.track_range(1000., 333.), (49., 133.));
    }
}
