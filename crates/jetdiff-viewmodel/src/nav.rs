//! Next/previous hunk. Port of `PrevNextDifferenceIterableBase`,
//! `SimpleDiffViewer.scrollToChange` and `SyncScrollSupport.getTargetOffsets`. No
//! animation: the target is the end position `doMakeVisible` lands on.

use jetdiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::FileDiff;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavTarget {
    /// The cursor is moved to the start of the hunk on both sides.
    pub cursor_lines: [usize; 2],
    pub scroll_y: [f32; 2],
    /// `doMakeVisible` scrolls the master to x = 0, and x is shared.
    pub scroll_x: f32,
}

/// `canGoNext` and `goNext`: the first hunk that starts below the cursor.
pub fn next_hunk(diff: &FileDiff, side: Side, cursor_line: usize) -> Option<usize> {
    let hunks = diff.hunks();
    let last = hunks.last()?;
    if cursor_line == diff.line_count(side) - 1 || last.lines(side).start <= cursor_line {
        return None;
    }
    hunks
        .iter()
        .position(|hunk| hunk.lines(side).start > cursor_line)
}

/// `canGoPrev` and `goPrev`: the last hunk that lies entirely above the cursor. If the cursor is inside a
/// hunk, it is the previous hunk, not the one you are in.
pub fn previous_hunk(diff: &FileDiff, side: Side, cursor_line: usize) -> Option<usize> {
    let hunks = diff.hunks();
    let first = hunks.first()?;
    let first = first.lines(side);
    if cursor_line == 0 || first.end > cursor_line || first.start >= cursor_line {
        return None;
    }
    (0..hunks.len()).find(|&i| match hunks.get(i + 1) {
        None => true,
        Some(next) => {
            let next = next.lines(side);
            next.end > cursor_line || next.start >= cursor_line
        }
    })
}

/// Hunks never lie in a fold, and `scrollToChange` opens nothing.
pub fn target_for_hunk(
    diff: &FileDiff,
    folding: &Folding,
    hunk: usize,
    viewport: Viewport,
) -> NavTarget {
    let lines = &diff.hunks()[hunk].lines;
    let layouts = folding.layouts(viewport.line_height);
    let starts = [lines[0].start, lines[1].start];
    let ends = [lines[0].end, lines[1].end];
    NavTarget {
        cursor_lines: starts,
        scroll_y: target_offsets(&layouts, starts, ends, viewport.height),
        scroll_x: 0.,
    }
}

/// `SyncScrollSupport.getTargetOffsets` with `preferredTopShift = -1`. Small hunks end up
/// 1/3 down, the same point as the sync anchor, so sync moves nothing after the jump.
pub(crate) fn target_offsets(
    layouts: &[LineLayout; 2],
    start_lines: [usize; 2],
    end_lines: [usize; 2],
    viewport_height: f32,
) -> [f32; 2] {
    let h = viewport_height;
    let mut top = [0.; 2];
    let mut range_h = [0.; 2];
    let mut gap = [0.; 2];
    let mut max = [0.; 2];
    let mut top_shifts = [0.; 2];
    for i in 0..2 {
        let l = &layouts[i];
        top[i] = l.line_top(start_lines[i]);
        // `endLine + 1` even though endLine is exclusive, so there is one extra line of space
        range_h[i] = l.line_top(end_lines[i] + 1) - top[i];
        gap[i] = 2. * l.line_height();
        max[i] = l.max_scroll(h);
        let can_show = 2. * gap[i] + range_h[i] <= h;
        let shift = (h / 3.).floor();
        top_shifts[i] = if can_show {
            (h - gap[i] - range_h[i]).min(shift)
        } else {
            gap[i]
        };
    }

    // The top of the file
    let top_shift = top_shifts[0].min(top_shifts[1]).min(top[0].min(top[1]));
    let mut offsets = top.map(|t| t - top_shift);
    if (0..2).all(|i| max[i] > offsets[i]) {
        return clamp(offsets, max);
    }

    // One of the sides is at the end of the file: show as much as possible, and always the top of the hunk
    let top_shift = (0..2).map(|i| top[i] - max[i]).fold(0f32, f32::max);
    for i in 0..2 {
        offsets[i] = top[i] - top_shift + (top_shift + range_h[i] + gap[i] - h).max(0.);
        offsets[i] = offsets[i].min(top[i] - gap[i]);
    }
    clamp(offsets, max)
}

/// The scrollbar clamps in IntelliJ.
fn clamp(offsets: [f32; 2], max: [f32; 2]) -> [f32; 2] {
    [offsets[0].clamp(0., max[0]), offsets[1].clamp(0., max[1])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    /// Hunks on lines 10, 40 and 90 of 100, one line each.
    fn three_hunks() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        FileDiff::new(&a, &(b.join("\n") + "\n"), ComparisonPolicy::Default).unwrap()
    }

    const VIEWPORT: Viewport = Viewport {
        height: 300.,
        line_height: 20.,
    };

    #[test]
    fn navigation_over_folds() {
        let diff = three_hunks();
        let folding = Folding::new(&diff);
        let mut seen = vec![0];
        let mut target = target_for_hunk(&diff, &folding, 0, VIEWPORT);
        loop {
            let hunk = *seen.last().unwrap();
            let start = diff.hunks()[hunk].lines[0].start;
            for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
                let layout = folding.layout(side, 20.);
                // The hunk is visible, and the folds above take one line each
                let y = layout.line_top(start) - target.scroll_y[i];
                assert!((0. ..VIEWPORT.height).contains(&y), "{target:?}");
                assert!(
                    layout.line_top(start) < Folding::none(&diff).layout(side, 20.).line_top(start)
                );
            }
            let Some(next) = next_hunk(&diff, Side::Right, target.cursor_lines[1]) else {
                break;
            };
            seen.push(next);
            target = target_for_hunk(&diff, &folding, next, VIEWPORT);
        }
        assert_eq!(seen, [0, 1, 2]);
        // Hunk 2 on line 90 is visual line 25 with three folds above
        assert_eq!(target.cursor_lines, [90, 90]);
        assert_eq!(target.scroll_y, [400., 400.]);
        assert_eq!(
            previous_hunk(&diff, Side::Right, target.cursor_lines[1]),
            Some(1)
        );
    }

    #[test]
    fn next_and_previous_from_cursor() {
        let diff = three_hunks();
        let s = Side::Right;
        assert_eq!(next_hunk(&diff, s, 0), Some(0));
        assert_eq!(next_hunk(&diff, s, 10), Some(1));
        assert_eq!(next_hunk(&diff, s, 39), Some(1));
        assert_eq!(next_hunk(&diff, s, 90), None);
        assert_eq!(next_hunk(&diff, s, 100), None);

        assert_eq!(previous_hunk(&diff, s, 0), None);
        assert_eq!(previous_hunk(&diff, s, 10), None);
        assert_eq!(previous_hunk(&diff, s, 11), Some(0));
        // Cursor in hunk 1 gives hunk 0
        assert_eq!(previous_hunk(&diff, s, 40), Some(0));
        assert_eq!(previous_hunk(&diff, s, 41), Some(1));
        assert_eq!(previous_hunk(&diff, s, 99), Some(2));
    }

    #[test]
    fn cursor_on_last_line_cannot_go_further() {
        // A hunk on the last line with content, cursor on the empty line after it
        let diff = FileDiff::new("a\nb\n", "a\nc\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(next_hunk(&diff, Side::Left, 0), Some(0));
        assert_eq!(next_hunk(&diff, Side::Left, 2), None);
    }

    #[test]
    fn no_hunks() {
        let diff = FileDiff::new("a\n", "a\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(next_hunk(&diff, Side::Left, 0), None);
        assert_eq!(previous_hunk(&diff, Side::Left, 1), None);
    }

    #[test]
    fn small_hunk_lands_a_third_down() {
        let diff = three_hunks();
        let target = target_for_hunk(&diff, &Folding::none(&diff), 1, VIEWPORT);
        assert_eq!(target.cursor_lines, [40, 40]);
        // shift = min(300 - 40 - 40, 100) = 100
        assert_eq!(target.scroll_y, [700., 700.]);
    }

    #[test]
    fn hunk_at_the_top() {
        let diff = three_hunks();
        // top = 200 above shift 100, so shift becomes 100
        assert_eq!(
            target_for_hunk(&diff, &Folding::none(&diff), 0, VIEWPORT).scroll_y,
            [100., 100.]
        );
        let diff = FileDiff::new("a\nb\n", "x\nb\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(
            target_for_hunk(&diff, &Folding::none(&diff), 0, VIEWPORT).scroll_y,
            [0., 0.]
        );
    }

    #[test]
    fn large_hunk_gets_two_lines_of_space() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 20),
            lines("x", 30),
            &a[lines("a", 20).len()..]
        );
        let diff = FileDiff::new(&a, &b, ComparisonPolicy::Default).unwrap();
        assert_eq!(diff.hunks()[0].lines, [20..20, 20..50]);
        // Right cannot be shown in full: shift = gap = 40 on both
        assert_eq!(
            target_for_hunk(&diff, &Folding::none(&diff), 0, VIEWPORT).scroll_y,
            [360., 360.]
        );
    }

    #[test]
    fn hunk_at_the_end_is_clamped() {
        let diff = three_hunks();
        // top = 1800, max = (101 + 5) * 20 - 300 = 1820
        let target = target_for_hunk(&diff, &Folding::none(&diff), 2, VIEWPORT);
        // First attempt 1700 < 1820, so enough room
        assert_eq!(target.scroll_y, [1700., 1700.]);

        let short = Viewport {
            height: 100.,
            line_height: 20.,
        };
        // Low window: shift = gap = 40 because 2*40 + 40 > 100
        assert_eq!(
            target_for_hunk(&diff, &Folding::none(&diff), 2, short).scroll_y,
            [1760., 1760.]
        );
    }

    #[test]
    fn fallback_when_one_side_is_at_the_end() {
        // Left short, Right long: the hunk is at the bottom of Left
        let a = lines("a", 10) + "tail\n";
        let b = lines("a", 10) + "tail2\n" + &lines("z", 50);
        let diff = FileDiff::new(&a, &b, ComparisonPolicy::Default).unwrap();
        let target = target_for_hunk(&diff, &Folding::none(&diff), 0, VIEWPORT);
        let left_side_max_scroll = Folding::none(&diff)
            .layout(Side::Left, 20.)
            .max_scroll(300.);
        assert_eq!(left_side_max_scroll, 40.);
        assert!(target.scroll_y[0] <= left_side_max_scroll);
        // The top of the hunk is visible on both sides
        for (i, s) in [Side::Left, Side::Right].into_iter().enumerate() {
            let top = Folding::none(&diff)
                .layout(s, 20.)
                .line_top(diff.hunks()[0].lines(s).start);
            assert!(top >= target.scroll_y[i] && top < target.scroll_y[i] + 300.);
        }
    }
}
