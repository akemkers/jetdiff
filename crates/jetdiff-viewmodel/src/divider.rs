//! The diff divider between the sides. Port of `DiffDividerDrawUtil` (`DividerPaintableHandlerImpl`,
//! `DividerPolygon.paint`) and `DiffDrawUtil.drawCurveTrapezium`. Coordinates are local to
//! the divider: x from 0 to the width, y from the top of the sides.

use crate::fold_paths::{PathSeg, fold_link_path};
use crate::folding::{FoldId, Folding};
use crate::layout::{LineLayout, Viewport};
use crate::model::{FileDiff, HunkKind};

/// `diff.divider.width` in the registry.
pub const DIVIDER_WIDTH: f32 = 24.;

/// `DiffDrawUtil.CTRL_PROXIMITY_X`.
const CTRL_PROXIMITY_X: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cubic {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
    pub p3: Point,
}

impl Cubic {
    /// `DiffDrawUtil.makeCurve(forward = true)`: from Left at `left_side_y` to Right at `right_side_y`.
    pub fn new(width: f32, left_side_y: f32, right_side_y: f32) -> Self {
        let p = |x, y| Point { x, y };
        Self {
            p0: p(0., left_side_y),
            p1: p(width * CTRL_PROXIMITY_X, left_side_y),
            p2: p(width * (1. - CTRL_PROXIMITY_X), right_side_y),
            p3: p(width, right_side_y),
        }
    }

    pub fn reversed(&self) -> Self {
        Self {
            p0: self.p3,
            p1: self.p2,
            p2: self.p1,
            p3: self.p0,
        }
    }
}

/// One hunk in the divider. Fill: `top`, line to `bottom.p3`, `bottom.reversed()`, close.
/// Also draw `mid` with a 1px stroke in the same color, so steep bands are never thinner than 1px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DividerBand {
    pub hunk: usize,
    /// Full color, also for hunks compared by word.
    pub kind: HunkKind,
    pub top: Cubic,
    pub bottom: Cubic,
    pub mid: Cubic,
}

/// Hunks that are visible on at least one side. `scroll_y` is per side, `[Left, Right]`.
pub fn divider_bands(
    diff: &FileDiff,
    folding: &Folding,
    scroll_y: [f32; 2],
    viewport: Viewport,
    width: f32,
) -> Vec<DividerBand> {
    let layouts = folding.layouts(viewport.line_height);
    let painted = [
        layouts[0].painted_lines(scroll_y[0], viewport.height),
        layouts[1].painted_lines(scroll_y[1], viewport.height),
    ];

    let above = |hunk: &crate::model::Hunk| {
        painted[0].0 > hunk.lines[0].end && painted[1].0 > hunk.lines[1].end
    };
    // `DividerPaintableHandlerImpl.process` skips hunks above the viewport. The ends
    // rise on both sides, so they can be skipped with a binary search.
    let first = diff.hunks().partition_point(above);
    let mut bands = Vec::new();
    for (i, hunk) in diff.hunks().iter().enumerate().skip(first) {
        let [l, r] = &hunk.lines;
        if painted[0].1 < l.start && painted[1].1 < r.start {
            break;
        }
        let side = |s: usize| trapezium_side(&layouts[s], hunk.lines[s].clone(), scroll_y[s]);
        let (left_side_start, left_side_end) = side(0);
        let (right_side_start, right_side_end) = side(1);
        bands.push(DividerBand {
            hunk: i,
            kind: hunk.kind,
            top: Cubic::new(width, left_side_start, right_side_start),
            bottom: Cubic::new(width, left_side_end + 1., right_side_end + 1.),
            // Java: `(start + end) / 2` in int, and a stroke on integer y covers the next half pixel
            mid: Cubic::new(
                width,
                mid_y(left_side_start, left_side_end),
                mid_y(right_side_start, right_side_end),
            ),
        });
    }
    bands
}

/// Fold link between two closed folds, stroked with `fold_paths::stroke_width(hovered)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldLink {
    pub id: FoldId,
    pub hovered: bool,
    pub path: Vec<PathSeg>,
}

/// `FoldingModelSupport.createVisibleSeparators`: fold links for folds that are painted on at least
/// one side.
pub fn fold_links(
    folding: &Folding,
    scroll_y: [f32; 2],
    viewport: Viewport,
    width: f32,
) -> Vec<FoldLink> {
    let lh = viewport.line_height;
    let layouts = folding.layouts(lh);
    let painted = [0, 1].map(|i| layouts[i].painted_lines(scroll_y[i], viewport.height));
    let mut result = Vec::new();
    for fold in folding.closed_folds() {
        let [left_side_line, right_side_line] = fold.lines;
        if painted[0].0 > left_side_line + 1 && painted[1].0 > right_side_line + 1 {
            continue;
        }
        if painted[0].1 < left_side_line && painted[1].1 < right_side_line {
            break;
        }
        let y = [0, 1].map(|i| layouts[i].line_top(fold.lines[i]) - scroll_y[i]);
        result.push(FoldLink {
            id: fold.id,
            hovered: fold.hovered,
            path: fold_link_path(width, y[0], y[1], lh),
        });
    }
    result
}

/// `DividerPolygon.paint`: under 2px height becomes `[start-1, start]`, otherwise `[start, end-1]`.
/// The fill goes to `end + 1`, so the band covers `[start, end)` or 2px around the boundary.
fn trapezium_side(layout: &LineLayout, lines: std::ops::Range<usize>, scroll_y: f32) -> (f32, f32) {
    let (y1, y2) = layout.marker_range(lines);
    let (y1, y2) = (y1 - scroll_y, y2 - scroll_y);
    if y2 - y1 < 2. {
        (y1 - 1., y1)
    } else {
        (y1, y2 - 1.)
    }
}

fn mid_y(start: f32, end: f32) -> f32 {
    ((start + end) / 2.).floor() + 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    const VP: Viewport = Viewport {
        height: 200.,
        line_height: 20.,
    };

    #[test]
    fn curve_has_control_points_at_30_and_70_percent() {
        let c = Cubic::new(24., 10., 50.);
        assert_eq!(c.p0, Point { x: 0., y: 10. });
        assert_eq!(
            c.p1,
            Point {
                x: 24. * 0.3,
                y: 10.
            }
        );
        assert_eq!(
            c.p2,
            Point {
                x: 24. * 0.7,
                y: 50.
            }
        );
        assert_eq!(c.p3, Point { x: 24., y: 50. });
        assert_eq!(c.reversed().p0, c.p3);
    }

    #[test]
    fn modified_hunk_with_known_numbers() {
        // Left lines 2..4, Right lines 2..5
        let d = diff("a\nb\nc\nd\ne\n", "a\nb\nx\ny\nz\ne\n");
        assert_eq!(d.hunks()[0].lines, [2..4, 2..5]);
        let bands = divider_bands(&d, &Folding::none(&d), [0., 10.], VP, 24.);
        assert_eq!(bands.len(), 1);
        let b = bands[0];
        assert_eq!(b.kind, HunkKind::Modified);
        assert_eq!((b.top.p0.y, b.top.p3.y), (40., 30.));
        assert_eq!((b.bottom.p0.y, b.bottom.p3.y), (80., 90.));
        // (40 + 79) / 2 = 59, (30 + 89) / 2 = 59
        assert_eq!((b.mid.p0.y, b.mid.p3.y), (59.5, 59.5));
    }

    #[test]
    fn empty_side_becomes_2px() {
        let d = diff("a\nb\n", "a\nx\ny\nb\n");
        assert_eq!(d.hunks()[0].lines, [1..1, 1..3]);
        let b = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
        assert_eq!(b.kind, HunkKind::Added);
        // Boundary at y=20: [19, 21)
        assert_eq!((b.top.p0.y, b.bottom.p0.y), (19., 21.));
        assert_eq!((b.top.p3.y, b.bottom.p3.y), (20., 60.));
        assert_eq!(b.mid.p0.y, 19.5);
    }

    #[test]
    fn empty_side_on_first_line_is_moved_down() {
        let d = diff("b\n", "x\nb\n");
        let b = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
        // `getGutterMarkerPaintRange` gives y=1 for line 0, so the band becomes [0, 2)
        assert_eq!((b.top.p0.y, b.bottom.p0.y), (0., 2.));
    }

    #[test]
    fn hunks_outside_viewport_are_cut() {
        // Hunks on lines 5, 30 and 60
        let a = lines("a", 80);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [5, 30, 60] {
            b[i] = format!("x{i}");
        }
        let b = b.join("\n") + "\n";
        let d = diff(&a, &b);
        assert_eq!(d.hunks().len(), 3);
        // Viewport covers lines 25..35
        let bands = divider_bands(&d, &Folding::none(&d), [500., 500.], VP, 24.);
        assert_eq!(
            bands.iter().map(|band| band.hunk).collect::<Vec<_>>(),
            vec![1]
        );
        // Visible on only one side is enough
        let bands = divider_bands(&d, &Folding::none(&d), [0., 500.], VP, 24.);
        assert_eq!(
            bands.iter().map(|band| band.hunk).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn hunk_right_below_the_edge_is_included() {
        // Closed interval: the line that starts on the bottom edge counts as painted
        let d = diff(
            &lines("a", 20),
            &(lines("a", 10) + "x\n" + &lines("a", 20)[lines("a", 11).len()..]),
        );
        assert_eq!(d.hunks()[0].lines, [10..11, 10..11]);
        assert_eq!(
            divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.).len(),
            1
        );
        assert_eq!(
            divider_bands(&d, &Folding::none(&d), [1., 1.], VP, 24.).len(),
            1
        );
    }

    #[test]
    fn fold_links_for_visible_folds() {
        // Hunks on lines 10, 40 and 90 of 100: folds 0..6, 15..36, 45..86 and 95..101
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        let f = Folding::new(&d);
        let links = fold_links(&f, [0., 0.], VP, 24.);
        // Viewport of 10 visual lines: fold 0 and 1 (visual line 0 and 10)
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].path[0], PathSeg::Move(Point { x: -0.5, y: 10. }));
        assert_eq!(links[1].path[0], PathSeg::Move(Point { x: -0.5, y: 210. }));
        // A partly visible fold is included, a fully hidden one is not
        let ids = |y: f32| -> Vec<usize> {
            let links = fold_links(&f, [y, y], VP, 24.);
            links.iter().map(|link| link.id).collect()
        };
        assert_eq!(ids(10.), [0, 1]);
        assert_eq!(ids(20.), [1]);
        assert_eq!(ids(210.), [1, 2]);
        assert_eq!(ids(230.), [2]);
        assert!(fold_links(&Folding::none(&d), [0., 0.], VP, 24.).is_empty());
    }

    #[test]
    fn large_diff_gives_only_visible_bands() {
        // 20k lines with a hunk every 50th line
        let a = lines("a", 20_000);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in (0..20_000).step_by(50) {
            b[i] = format!("x{i}");
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        assert_eq!(d.hunks().len(), 400);
        let y = 10_000. * VP.line_height;
        let bands = divider_bands(&d, &Folding::none(&d), [y, y], VP, 24.);
        assert_eq!(
            bands.iter().map(|band| band.hunk).collect::<Vec<_>>(),
            vec![200]
        );
    }
}
