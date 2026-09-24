//! Error stripe i scrollbaren. Diff-highlighterne har `ERROR_STRIPE_COLOR` over
//! `DiffUtil.getLinesRange(start, end, false)`, og posisjonen regnes ut som i
//! `EditorMarkupModelImpl.offsetsToYPositions`.

use smudiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::LineLayout;
use crate::model::{DiffType, FileDiff};

/// `DaemonCodeAnalyzerSettings.myErrorStripeMarkMinHeight`.
const MIN_MARK_HEIGHT: f32 = 2.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripeMarker {
    pub diff_type: DiffType,
    /// Y i editoren, piksler fra toppen av dokumentet.
    pub start_y: f32,
    pub end_y: f32,
}

impl StripeMarker {
    /// Piksler i sporet. Er dokumentet lavere enn sporet mappes det 1:1, ellers skaleres det
    /// og kuttes til heltall. Minst 2px høy.
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

/// Tommelen i sporet, `None` når alt innhold får plass. Proporsjonal som en Swing-scrollbar
/// med `content_height` som maks og viewporten som extent.
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

/// Med folds: y er visuell, som `offsetToY` i `EditorMarkupModelImpl`.
pub fn stripe_markers(
    diff: &FileDiff,
    folding: &Folding,
    side: Side,
    line_height: f32,
) -> Vec<StripeMarker> {
    let layout = folding.layout(side, line_height);
    diff.blocks()
        .iter()
        .map(|block| {
            let (start_y, end_y) = marker_y(&layout, block.lines(side));
            StripeMarker {
                diff_type: block.diff_type,
                start_y,
                end_y,
            }
        })
        .collect()
}

/// Slutt-offset er uten siste linjeskift, så sluttlinja er `end - 1` og markøren slutter
/// på toppen av den. Tom range ligger på `start`, klemt til siste linje.
fn marker_y(layout: &LineLayout, lines: std::ops::Range<usize>) -> (f32, f32) {
    let last = layout.line_count() - 1;
    let start = lines.start.min(last);
    let end = if lines.is_empty() {
        start
    } else {
        lines.end - 1
    };
    let start_y = layout.line_top(start);
    // `startLineNumber == endLineNumber` gir `endY = startY`
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
    use smudiff_core::ComparisonPolicy;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    #[test]
    fn markorer_med_folds_er_visuelle() {
        let a: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let b = a.replace("a50\n", "x\n");
        let d = diff(&a, &b);
        let f = Folding::new(&d, crate::folding::FoldSettings::default());
        let m = stripe_markers(&d, &f, Side::Left, 10.);
        // 0..46 foldes til én linje, så linje 50 er visuell linje 5
        assert_eq!(f.layout(Side::Left, 10.).logical_to_visual(50), 5);
        assert_eq!(m[0].start_y, 50.);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn markorer_per_side() {
        // Venstre linje 1..2 slettet, høyre har tom side på linje 1
        let d = diff("a\nb\nc\nd\ne\n", "a\nc\nx\ny\nz\n");
        let left = stripe_markers(&d, &Folding::none(&d), Side::Left, 10.);
        let right = stripe_markers(&d, &Folding::none(&d), Side::Right, 10.);
        assert_eq!(d.blocks()[0].lines, [1..2, 1..1]);
        assert_eq!((left[0].start_y, left[0].end_y), (10., 10.));
        assert_eq!((right[0].start_y, right[0].end_y), (10., 10.));
        assert_eq!(d.blocks()[1].lines, [3..5, 2..5]);
        // Høyre 2..5: topp av linje 2 til topp av linje 4
        assert_eq!((right[1].start_y, right[1].end_y), (20., 40.));
        assert_eq!(right[1].diff_type, DiffType::Modified);
    }

    #[test]
    fn tom_range_etter_siste_linje_klemmes() {
        // Venstre mangler '\n' på slutten, blokken ligger på linje 3 av 3
        let d = diff("one\ntwo\nthree", "one\ntwo\nthree\n");
        assert_eq!(d.blocks()[0].lines, [3..3, 3..4]);
        let left = stripe_markers(&d, &Folding::none(&d), Side::Left, 10.);
        assert_eq!((left[0].start_y, left[0].end_y), (20., 20.));
    }

    #[test]
    fn tommel_er_proporsjonal() {
        assert_eq!(thumb_range(0., 500., 400., 500.), None);
        assert_eq!(thumb_range(0., 250., 1000., 500.), Some((0., 125.)));
        assert_eq!(thumb_range(750., 250., 1000., 500.), Some((375., 500.)));
    }

    #[test]
    fn sporet_skalerer_og_har_minstehoyde() {
        let m = StripeMarker {
            diff_type: DiffType::Inserted,
            start_y: 150.,
            end_y: 150.,
        };
        assert_eq!(m.track_range(300., 600.), (150., 152.));
        assert_eq!(m.track_range(1000., 333.), (49., 51.));
        let m = StripeMarker { end_y: 400., ..m };
        assert_eq!(m.track_range(1000., 333.), (49., 133.));
    }
}
