//! Sync scroll. Port of `BaseSyncScrollable`, `SimpleDiffViewer.MySyncScrollable` and
//! `SyncScrollSupport.ScrollHelper.syncVerticalScroll`. Scroll y is pixels from the top of
//! the document, per side.

use jetdiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::{FileDiff, Hunk, idx};

mod code_view_keeper;

pub(crate) use code_view_keeper::CodeViewKeeper;

/// `Range` from IntelliJ seen from the master: 1 is the master, 2 is the slave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SyncRange {
    start1: usize,
    end1: usize,
    start2: usize,
    end2: usize,
}

/// The boundary points `(0,0)`, start and end of each hunk, and `(lineCount1, lineCount2)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMapping {
    bounds: Vec<[usize; 2]>,
    has_hunks: bool,
}

impl LineMapping {
    pub fn new(hunks: &[Hunk], line_counts: [usize; 2]) -> Self {
        let mut bounds = vec![[0, 0]];
        for hunk in hunks {
            bounds.push([hunk.lines[0].start, hunk.lines[1].start]);
            bounds.push([hunk.lines[0].end, hunk.lines[1].end]);
        }
        bounds.push(line_counts);
        Self {
            bounds,
            has_hunks: !hunks.is_empty(),
        }
    }

    /// `BaseSyncScrollable.getRange` with `ScrollHelper.process`: the neighboring boundaries that enclose the line.
    fn range(&self, master: Side, line: usize) -> SyncRange {
        if !self.has_hunks {
            return id_range(line);
        }
        let (m, s) = (idx(master), idx(master.other()));
        // The first boundary with `line <= b[m]`, otherwise the last one. Binary search so scroll doesn't become
        // O(hunks) per frame.
        let i = self
            .bounds
            .partition_point(|b| b[m] < line)
            .min(self.bounds.len() - 1);
        let prev = if i == 0 { [0, 0] } else { self.bounds[i - 1] };
        let cur = self.bounds[i];
        SyncRange {
            start1: prev[m],
            end1: cur[m],
            start2: prev[s],
            end2: cur[s],
        }
    }

    pub fn transfer(&self, master: Side, line: usize) -> usize {
        transfer_line(line, self.range(master, line))
    }
}

/// `BaseSyncScrollable.transferLine`. Inside a hunk the slave goes line by line from the top
/// and stands still when the short side is used up.
fn transfer_line(line: usize, r: SyncRange) -> usize {
    if r.start1 == line {
        return r.start2;
    }
    if r.end1 == line {
        return r.end2;
    }
    if r.end1 < line {
        return line - r.end1 + r.end2;
    }
    (r.start2 + (line - r.start1)).min(r.end2)
}

fn id_range(line: usize) -> SyncRange {
    SyncRange {
        start1: line,
        end1: line + 1,
        start2: line,
        end2: line + 1,
    }
}

/// `ScrollHelper.transferVisualLine`: the boundaries are converted to visual lines on each
/// side, so folded areas come out right without special code.
fn transfer_visual_line(
    mapping: &LineMapping,
    master: Side,
    master_layout: &LineLayout,
    slave_layout: &LineLayout,
    master_visual: usize,
) -> usize {
    let logical = master_layout.visual_to_logical(master_visual);
    let r = mapping.range(master, logical);
    let visual = |layout: &LineLayout, start: usize, end: usize| {
        let s = layout.logical_to_visual(start);
        let e = if start == end {
            s
        } else {
            layout.logical_to_visual(end)
        };
        (s, e)
    };
    let (start1, end1) = visual(master_layout, r.start1, r.end1);
    let (start2, end2) = visual(slave_layout, r.start2, r.end2);
    transfer_line(
        master_visual,
        SyncRange {
            start1,
            end1,
            start2,
            end2,
        },
    )
}

/// `ScrollHelper.syncVerticalScroll` and `doScrollVertically` without anchor and animation.
/// The anchor is 1/3 down the viewport. When the slave is in an empty hunk side it only moves
/// by whole lines, so it doesn't vibrate while the master scrolls through a pure insert.
fn sync_vertical_scroll(
    mapping: &LineMapping,
    master: Side,
    master_layout: &LineLayout,
    slave_layout: &LineLayout,
    master_y: f32,
    slave_y: f32,
    viewport_height: f32,
) -> f32 {
    let lh = master_layout.line_height();
    let transfer =
        |visual| transfer_visual_line(mapping, master, master_layout, slave_layout, visual);

    // Integer division in Java
    let middle_y = (viewport_height / 3.).floor();
    let master_offset = master_y + middle_y;
    let master_visual = master_layout.y_to_visual_line(master_offset);
    let slave_visual = transfer(master_visual);

    let slave_offset = slave_layout.visual_line_to_y(slave_visual);
    let master_offset_raw = master_layout.visual_line_to_y(master_visual);
    // Keeps the anchor lines in the same phase
    let correction = (master_offset - master_offset_raw) % lh;
    let offset = slave_offset - middle_y + correction;

    let half = (lh / 2.).floor();
    let only_major_backward =
        correction < half && master_visual > 0 && slave_visual == transfer(master_visual - 1);
    let only_major_forward = correction > half && slave_visual == transfer(master_visual + 1);

    let target = if (only_major_forward && offset > slave_y
        || only_major_backward && offset < slave_y)
        && (offset - slave_y).abs() < slave_layout.line_height()
    {
        slave_y
    } else {
        offset
    };
    slave_layout.clamp_scroll(target, viewport_height)
}

/// Scroll for both sides. Horizontal scroll is shared, as in the spike and JetBrains
/// (`syncHorizontalScroll` sets the same x).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollState {
    pub y: [f32; 2],
    pub x: f32,
}

impl ScrollState {
    pub fn y(&self, side: Side) -> f32 {
        self.y[idx(side)]
    }

    /// The side the user scrolls is the master. The slave is computed in the same frame.
    pub fn scroll_to(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        y: f32,
        viewport: Viewport,
    ) {
        let layout = folding.layout(master, viewport.line_height);
        let y = layout.clamp_scroll(y, viewport.height);
        // `visibleAreaChanged` only syncs when y actually changed
        if y == self.y(master) {
            return;
        }
        self.y[idx(master)] = y;
        self.sync_from(diff, folding, master, viewport);
    }

    pub fn scroll_by(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        dy: f32,
        viewport: Viewport,
    ) {
        self.scroll_to(diff, folding, master, self.y(master) + dy, viewport);
    }

    /// Recomputes the slave, e.g. after a changed viewport height.
    pub fn sync_from(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        viewport: Viewport,
    ) {
        let slave = master.other();
        let master_layout = folding.layout(master, viewport.line_height);
        let slave_layout = folding.layout(slave, viewport.line_height);
        self.y[idx(slave)] = sync_vertical_scroll(
            diff.mapping(),
            master,
            &master_layout,
            &slave_layout,
            self.y(master),
            self.y(slave),
            viewport.height,
        );
    }

    /// `max_x` comes from the UI, which knows the text width.
    pub fn scroll_x_to(&mut self, x: f32, max_x: f32) {
        self.x = x.clamp(0., max_x.max(0.));
    }
}

/// `CodeViewKeeper` per side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionKeeper {
    sides: [CodeViewKeeper; 2],
}

impl PositionKeeper {
    pub fn save(
        state: &ScrollState,
        folding: &Folding,
        cursor_lines: [usize; 2],
        viewport: Viewport,
    ) -> Self {
        let sides = [Side::Left, Side::Right].map(|side| {
            let layout = folding.layout(side, viewport.line_height);
            let i = idx(side);
            CodeViewKeeper::save(&layout, state.y[i], cursor_lines[i], viewport)
        });
        Self { sides }
    }

    /// `cursor_lines` are after the change, moved out of closed folds. Left is restored
    /// first without sync, since Right is still in the batch. If Right moves, it syncs
    /// Left as in `runBatchOperation`.
    pub fn restore(
        &self,
        state: &mut ScrollState,
        diff: &FileDiff,
        folding: &Folding,
        cursor_lines: [usize; 2],
        viewport: Viewport,
    ) {
        let before = state.y[1];
        for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let layout = folding.layout(side, viewport.line_height);
            state.y[i] = self.sides[i].restore(&layout, cursor_lines[i], viewport);
        }
        if state.y[1] != before {
            state.sync_from(diff, folding, Side::Right, viewport);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    const L: Side = Side::Left;
    const R: Side = Side::Right;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    fn r(start1: usize, end1: usize, start2: usize, end2: usize) -> SyncRange {
        SyncRange {
            start1,
            end1,
            start2,
            end2,
        }
    }

    #[test]
    fn transfer_line_edges() {
        let range = r(10, 20, 30, 33);
        assert_eq!(transfer_line(10, range), 30);
        assert_eq!(transfer_line(20, range), 33);
        assert_eq!(transfer_line(11, range), 31);
        assert_eq!(transfer_line(12, range), 32);
        // Clamped when the short side is used up
        assert_eq!(transfer_line(13, range), 33);
        assert_eq!(transfer_line(19, range), 33);
        // After the last boundary
        assert_eq!(transfer_line(25, range), 38);
    }

    #[test]
    fn transfer_line_empty_hunk() {
        // Pure insert on the slave side: the master stands still on the boundary
        assert_eq!(transfer_line(5, r(5, 5, 5, 9)), 5);
        // Pure insert on the master: the whole hunk maps to the boundary
        let range = r(5, 9, 5, 5);
        for line in 5..=9 {
            assert_eq!(transfer_line(line, range), 5);
        }
    }

    fn mapping_for(a: &str, b: &str) -> (FileDiff, Vec<Hunk>) {
        let d = diff(a, b);
        let hunks = d.hunks().to_vec();
        (d, hunks)
    }

    #[test]
    fn transfer_before_after_and_inside_hunk() {
        // Left 0..10, Right has 3 extra lines after line 4
        let a = lines("a", 10);
        let b = format!(
            "{}{}{}",
            lines("a", 4),
            lines("x", 3),
            &lines("a", 10)[lines("a", 4).len()..]
        );
        let (d, hunks) = mapping_for(&a, &b);
        assert_eq!(hunks[0].lines, [4..4, 4..7]);
        let m = d.mapping();
        assert_eq!(m.transfer(L, 0), 0);
        assert_eq!(m.transfer(L, 3), 3);
        assert_eq!(m.transfer(L, 4), 4);
        assert_eq!(m.transfer(L, 5), 8);
        // Left has 11 lines including the empty one after the last '\n'
        assert_eq!(m.transfer(L, 10), 13);
        // Past the end, linearly
        assert_eq!(m.transfer(L, 14), 17);

        assert_eq!(m.transfer(R, 4), 4);
        assert_eq!(m.transfer(R, 5), 4);
        assert_eq!(m.transfer(R, 6), 4);
        assert_eq!(m.transfer(R, 7), 4);
        assert_eq!(m.transfer(R, 8), 5);
    }

    #[test]
    fn transfer_start_and_end_of_document() {
        // Insert at the very start and end
        let a = "m\n";
        let b = "x\ny\nm\nz\n";
        let d = diff(a, b);
        let m = d.mapping();
        assert_eq!(d.hunks()[0].lines, [0..0, 0..2]);
        assert_eq!(m.transfer(L, 0), 0);
        assert_eq!(m.transfer(R, 0), 0);
        assert_eq!(m.transfer(R, 1), 0);
        assert_eq!(m.transfer(R, 2), 0);
        assert_eq!(m.transfer(L, 1), d.hunks()[1].lines[1].start);
        assert_eq!(m.transfer(L, 2), 5);
        assert_eq!(m.transfer(R, 5), 2);
    }

    #[test]
    fn binary_search_gives_same_range_as_linear_search() {
        let a = lines("a", 60);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        b[3] = "x".into();
        b.insert(20, "y".into());
        b.insert(20, "z".into());
        b.remove(40);
        b.truncate(55);
        let d = diff(&a, &(b.join("\n") + "\n"));
        let m = d.mapping();
        for master in [L, R] {
            let (mi, si) = (idx(master), idx(master.other()));
            for line in 0..70 {
                let (mut prev, mut cur) = ([0, 0], [0, 0]);
                for &bound in &m.bounds {
                    prev = cur;
                    cur = bound;
                    if line <= bound[mi] {
                        break;
                    }
                }
                let expected = r(prev[mi], cur[mi], prev[si], cur[si]);
                assert_eq!(m.range(master, line), expected, "{master:?} {line}");
            }
        }
    }

    #[test]
    fn without_hunks_mapping_is_identity() {
        let d = diff("a\nb\n", "a\nb\n");
        assert_eq!(d.mapping().transfer(L, 1), 1);
        assert_eq!(d.mapping().transfer(R, 40), 40);
    }

    const VP: Viewport = Viewport {
        height: 300.,
        line_height: 20.,
    };

    fn sync(d: &FileDiff, master: Side, master_y: f32, slave_y: f32) -> f32 {
        sync_folded(d, &Folding::none(d), master, master_y, slave_y)
    }

    fn sync_folded(d: &FileDiff, f: &Folding, master: Side, master_y: f32, slave_y: f32) -> f32 {
        let (ml, sl) = (f.layout(master, 20.), f.layout(master.other(), 20.));
        sync_vertical_scroll(d.mapping(), master, &ml, &sl, master_y, slave_y, VP.height)
    }

    /// Hunks on lines 10, 40 and 90 of 100. Closed folds 0..6, 15..36, 45..86 and
    /// 95..101 on both sides.
    fn three_hunks() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        diff(&a, &(b.join("\n") + "\n"))
    }

    fn folded(d: &FileDiff) -> Folding {
        Folding::new(d)
    }

    #[test]
    fn opening_keeps_cursor_in_place() {
        let d = three_hunks();
        let mut f = folded(&d);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 400., VP);
        assert_eq!(s.y, [400., 400.]);
        let cursor_lines = [90, 90];
        let screen = |f: &Folding, s: &ScrollState| {
            [L, R].map(|side| f.layout(side, 20.).line_top(90) - s.y(side))
        };
        let before = screen(&f, &s);
        let keeper = PositionKeeper::save(&s, &f, cursor_lines, VP);
        let id = f.fold_at_line(L, 45).unwrap();
        f.set_open(id, true);
        keeper.restore(&mut s, &d, &f, cursor_lines, VP);
        // 45..86 opened completely, so 40 more visual lines above the cursor
        assert_eq!(s.y, [1200., 1200.]);
        assert_eq!(screen(&f, &s), before);
    }

    #[test]
    fn cursor_outside_keeps_top_line() {
        let d = three_hunks();
        let mut f = folded(&d);
        let mut s = ScrollState::default();
        // Visual line 11 is logical 36
        s.scroll_to(&d, &f, L, 220., VP);
        let cursor_lines = [0, 0];
        let keeper = PositionKeeper::save(&s, &f, cursor_lines, VP);
        // 0..6 opens, so five more visual lines
        let id = f.fold_at_line(R, 0).unwrap();
        f.set_open(id, true);
        keeper.restore(&mut s, &d, &f, cursor_lines, VP);
        assert_eq!(s.y, [320., 320.]);
        assert_eq!(f.layout(L, 20.).y_to_logical_line(s.y[0]), 36);

        // The top is on a fold that opens: the first line stays put
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 500., VP);
        assert_eq!(f.layout(L, 20.).y_to_logical_line(500.), 45);
        let keeper = PositionKeeper::save(&s, &f, cursor_lines, VP);
        f.open_at(L, 45);
        keeper.restore(&mut s, &d, &f, cursor_lines, VP);
        assert_eq!(s.y, [500., 500.]);
    }

    #[test]
    fn closing_moves_cursor_to_the_fold() {
        let d = three_hunks();
        let mut f = folded(&d);
        f.set_all_open(true);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 1000., VP);
        let cursor_lines = [60, 60];
        let keeper = PositionKeeper::save(&s, &f, cursor_lines, VP);
        f.set_all_open(false);
        let cursor_lines = cursor_lines.map(|line| f.cursor_after(L, line));
        assert_eq!(cursor_lines, [45, 45]);
        keeper.restore(&mut s, &d, &f, cursor_lines, VP);
        // The fold ends up where the cursor line was
        assert_eq!(f.layout(L, 20.).line_top(45) - s.y[0], 1200. - 1000.);
    }

    #[test]
    fn sync_with_different_folds() {
        // Right has 30 extra lines at 40, so the runs and folds differ
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        b[10] = "x".into();
        b[90] = "y".into();
        for i in 0..30 {
            b.insert(40, format!("n{i}"));
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        let f = folded(&d);
        let (ml, sl) = (f.layout(L, 20.), f.layout(R, 20.));
        assert_ne!(ml.folds(), sl.folds());
        let anchor = (VP.height / 3.).floor();
        let mut checked = 0;
        let mut y = 0.;
        while y <= ml.max_scroll(VP.height) {
            let slave = sync_folded(&d, &f, L, y, 0.);
            let master_line = ml.y_to_logical_line(y + anchor);
            let in_hunk = d
                .hunks()
                .iter()
                .any(|hunk| hunk.lines[0].contains(&master_line));
            let in_fold = ml
                .folds()
                .iter()
                .any(|fold| fold.lines.contains(&master_line));
            let clamped = slave == 0. || slave == sl.max_scroll(VP.height);
            if !in_hunk && !in_fold && !clamped {
                let slave_line = sl.y_to_logical_line(slave + anchor);
                assert_eq!(slave_line, d.mapping().transfer(L, master_line), "y={y}");
                checked += 1;
            }
            y += 5.;
        }
        assert!(checked > 20);
    }

    #[test]
    fn equal_files_scroll_the_same() {
        let a = lines("a", 100);
        let d = diff(&a, &a);
        for y in [0., 7., 20., 333.5, 1000.] {
            assert_eq!(sync(&d, L, y, 0.), y);
        }
    }

    #[test]
    fn anchor_a_third_down() {
        // 10 extra lines on Right after line 50
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        assert_eq!(d.hunks()[0].lines, [50..50, 50..60]);
        // Anchor (y+100) above the hunk: the same
        assert_eq!(sync(&d, L, 800., 800.), 800.);
        // Anchor below the hunk: shifted 10 lines
        assert_eq!(sync(&d, L, 1100., 0.), 1300.);
        // From Right: the anchor inside the added hunk hits the boundary on Left
        assert_eq!(sync(&d, R, 950., 0.), 50. * 20. - 100. + 50. % 20.);
    }

    #[test]
    fn slave_moves_only_whole_lines_in_empty_hunk() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        // Master = Right, the anchor (y + 100) goes through the added lines 50..60.
        // The slave (Left) follows the phase to the middle of the first line and then stands still
        // until the anchor is halfway into the line after the hunk.
        let mut slave = 0.;
        for y in 880..1140 {
            let y = y as f32;
            let prev = slave;
            slave = sync(&d, R, y, slave);
            assert!(slave >= prev, "slave went backward at master {y}");
            let expected = match y {
                ..=910. => y,
                ..1110. => 910.,
                _ => y - 200.,
            };
            assert_eq!(slave, expected, "master {y}");
        }
    }

    #[test]
    fn slave_is_clamped() {
        // Left short, Right long: the slave cannot go past its max
        let a = lines("a", 5);
        let b = lines("a", 200);
        let d = diff(&a, &b);
        let right_side_max_scroll = Folding::none(&d).layout(R, 20.).max_scroll(VP.height);
        assert_eq!(
            sync(&d, R, right_side_max_scroll, 0.),
            Folding::none(&d).layout(L, 20.).max_scroll(VP.height)
        );
        assert_eq!(sync(&d, L, 0., 500.), 0.);
    }

    #[test]
    fn scroll_state_switches_master() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &Folding::none(&d), L, 1100., VP);
        assert_eq!(s.y, [1100., 1300.]);
        s.scroll_by(&d, &Folding::none(&d), R, -1300., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, &Folding::none(&d), L, -50., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, &Folding::none(&d), L, 1e9, VP);
        assert_eq!(
            s.y(L),
            Folding::none(&d).layout(L, 20.).max_scroll(VP.height)
        );
        s.scroll_x_to(-4., 100.);
        assert_eq!(s.x, 0.);
        s.scroll_x_to(400., 100.);
        assert_eq!(s.x, 100.);
    }
}
