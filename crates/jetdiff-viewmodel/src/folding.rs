//! Folding of unchanged lines. Port of `FoldingModelSupport`, where `FoldedBlock` holds the regions
//! paired on both sides. IntelliJ nests three folds per run of unchanged lines, jetdiff builds one
//! (ADR-0002).

use std::ops::Range;

use jetdiff_core::side::Side;

use crate::layout::{LineLayout, VisualFold, visual_folds};
use crate::model::{FileDiff, idx};

/// Lines shown around each hunk, like the default in `TextDiffSettings.contextRange`.
const CONTEXT_RANGE: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldBlock {
    /// `None` when the side has fewer than two lines to fold.
    pub lines: [Option<Range<usize>>; 2],
    /// Shared by the sides, since `MyFoldingListener` mirrors the state.
    pub expanded: bool,
}

impl FoldBlock {
    pub fn lines(&self, side: Side) -> Option<Range<usize>> {
        self.lines[idx(side)].clone()
    }
}

/// Index into `Folding::folds`.
pub type FoldId = usize;

/// A collapsed fold that is shown, with an id for click and hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connector {
    pub id: FoldId,
    /// The first line of the fold per side.
    pub lines: [usize; 2],
    pub hovered: bool,
}

type Regions = [Option<Range<usize>>; 2];

#[derive(Debug, Clone)]
pub struct Folding {
    line_counts: [usize; 2],
    /// One per run of unchanged lines, in line order.
    folds: Vec<FoldBlock>,
    visible: [Vec<VisualFold>; 2],
    visible_ids: [Vec<FoldId>; 2],
    hovered: Option<FoldId>,
}

impl Folding {
    pub fn new(diff: &FileDiff) -> Self {
        let line_counts = line_counts(diff);
        let hunks = diff.hunks().iter().map(|hunk| hunk.lines.clone());
        let folds = build_folds(hunks, line_counts)
            .into_iter()
            .map(|lines| FoldBlock {
                lines,
                expanded: false,
            })
            .collect();
        Self::with_folds(line_counts, folds)
    }

    /// No folds. For tests.
    pub fn none(diff: &FileDiff) -> Self {
        Self::with_folds(line_counts(diff), Vec::new())
    }

    fn with_folds(line_counts: [usize; 2], folds: Vec<FoldBlock>) -> Self {
        let mut folding = Self {
            line_counts,
            folds,
            visible: Default::default(),
            visible_ids: Default::default(),
            hovered: None,
        };
        folding.update();
        folding
    }

    pub fn folds(&self) -> &[FoldBlock] {
        &self.folds
    }

    pub fn layout(&self, side: Side, line_height: f32) -> LineLayout<'_> {
        LineLayout::with_folds(
            self.line_counts[idx(side)],
            line_height,
            &self.visible[idx(side)],
        )
    }

    pub fn layouts(&self, line_height: f32) -> [LineLayout<'_>; 2] {
        [
            self.layout(Side::Left, line_height),
            self.layout(Side::Right, line_height),
        ]
    }

    /// Collapsed folds shown on the side, in order.
    pub fn visible(&self, side: Side) -> impl Iterator<Item = (FoldId, &VisualFold)> {
        let i = idx(side);
        self.visible_ids[i].iter().copied().zip(&self.visible[i])
    }

    /// `getBlockForLine`: the collapsed fold that starts on the line. Other lines in a fold are
    /// hidden, and `yPositionToLogicalLine` always gives the first line.
    pub fn fold_at_line(&self, side: Side, line: usize) -> Option<FoldId> {
        let i = idx(side);
        let folds = &self.visible[i];
        let k = folds.partition_point(|f| f.lines.start < line);
        folds
            .get(k)
            .filter(|f| f.lines.start == line)
            .map(|_| self.visible_ids[i][k])
    }

    /// `FoldedBlock.setExpanded`: both sides.
    pub fn set_expanded(&mut self, id: FoldId, expanded: bool) -> bool {
        let fold = &mut self.folds[id];
        if fold.expanded == expanded {
            return false;
        }
        fold.expanded = expanded;
        self.update();
        true
    }

    /// `FoldingModelSupport.expandAll`.
    pub fn expand_all(&mut self, expanded: bool) -> bool {
        let mut changed = false;
        for fold in &mut self.folds {
            changed |= fold.expanded != expanded;
            fold.expanded = expanded;
        }
        self.update();
        changed
    }

    /// `ExpandRegionAction` at the caret: the closed fold that contains the line.
    pub fn expand_at(&mut self, side: Side, line: usize) -> bool {
        let found = self.folds.iter().position(|fold| {
            !fold.expanded && fold.lines(side).is_some_and(|lines| lines.contains(&line))
        });
        found.is_some_and(|id| self.set_expanded(id, true))
    }

    /// `FoldingModelImpl.onFoldProcessingEnd`: a caret inside a collapsed fold is moved to
    /// its start.
    pub fn caret_after(&self, side: Side, line: usize) -> usize {
        let folds = &self.visible[idx(side)];
        let k = folds.partition_point(|f| f.lines.start <= line);
        match k.checked_sub(1).map(|k| &folds[k]) {
            Some(f) if line < f.lines.end => f.lines.start,
            _ => line,
        }
    }

    pub fn hovered(&self) -> Option<FoldId> {
        self.hovered
    }

    pub fn set_hovered(&mut self, id: Option<FoldId>) -> bool {
        let changed = self.hovered != id;
        self.hovered = id;
        changed
    }

    /// `MyPaintable.process`: the folds that are closed on both sides. If one side has no fold,
    /// there is no fold link.
    pub fn connectors(&self) -> impl Iterator<Item = Connector> + '_ {
        self.folds.iter().enumerate().filter_map(|(id, fold)| {
            let [Some(left_side_lines), Some(right_side_lines)] = &fold.lines else {
                return None;
            };
            (!fold.expanded).then_some(Connector {
                id,
                lines: [left_side_lines.start, right_side_lines.start],
                hovered: self.hovered == Some(id),
            })
        })
    }

    /// The closed folds per side, which `FoldingModel` shows with a separator
    /// (`getHighlighterCondition`).
    fn update(&mut self) {
        for side in [Side::Left, Side::Right] {
            let i = idx(side);
            let (ids, ranges): (Vec<_>, Vec<_>) = self
                .folds
                .iter()
                .enumerate()
                .filter(|(_, fold)| !fold.expanded)
                .filter_map(|(id, fold)| Some((id, fold.lines(side)?)))
                .unzip();
            self.visible[i] = visual_folds(ranges);
            self.visible_ids[i] = ids;
        }
        if self
            .hovered
            .is_some_and(|id| !self.visible_ids.iter().flatten().any(|&v| v == id))
        {
            self.hovered = None;
        }
    }
}

fn line_counts(diff: &FileDiff) -> [usize; 2] {
    [diff.line_count(Side::Left), diff.line_count(Side::Right)]
}

/// `FoldingBuilderBase.build` and `addGroup` with only the first level: one fold per run of
/// unchanged lines between the hunks, plus before the first and after the last.
fn build_folds(
    hunks: impl IntoIterator<Item = [Range<usize>; 2]>,
    line_counts: [usize; 2],
) -> Vec<Regions> {
    let mut folds = Vec::new();
    let counts = line_counts.map(|c| c as i64);
    let context = CONTEXT_RANGE as i64;
    let mut add = |starts: [i64; 2], ends: [i64; 2]| {
        let regions: Regions = std::array::from_fn(|i| {
            let start = starts[i].saturating_add(context).clamp(0, counts[i]);
            let end = ends[i].saturating_sub(context).clamp(0, counts[i]);
            (end - start >= 2).then_some(start as usize..end as usize)
        });
        if regions.iter().any(Option::is_some) {
            folds.push(regions);
        }
    };
    // `Integer.MIN_VALUE` and `MAX_VALUE`: the first and last fold go all the way to the edge
    let mut last = [i64::MIN; 2];
    for lines in hunks {
        let starts = last;
        last = [lines[0].end as i64, lines[1].end as i64];
        add(starts, [lines[0].start as i64, lines[1].start as i64]);
    }
    add(last, [i64::MAX; 2]);
    folds
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

    /// Hunks on lines 10, 40 and 90 of 100.
    fn three_hunks() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        diff(&a, &(b.join("\n") + "\n"))
    }

    fn both(r: Range<usize>) -> Regions {
        [Some(r.clone()), Some(r)]
    }

    fn visible(folding: &Folding, side: Side) -> Vec<Range<usize>> {
        folding
            .visible(side)
            .map(|(_, fold)| fold.lines.clone())
            .collect()
    }

    #[test]
    fn one_fold_per_run_with_four_lines_of_context() {
        let f = Folding::new(&three_hunks());
        let folds: Vec<_> = f.folds().iter().map(|fold| fold.lines.clone()).collect();
        assert_eq!(
            folds,
            // 101 lines with the empty one after the last '\n'. The first and last fold go all the
            // way to the edge.
            [both(0..6), both(15..36), both(45..86), both(95..101)]
        );
        assert_eq!(visible(&f, L), [0..6, 15..36, 45..86, 95..101]);
    }

    #[test]
    fn at_least_two_lines_per_side() {
        // Run of 9 lines: 4 context on each side leaves one line, so no fold
        let folds = build_folds([[0..1, 0..1], [10..11, 10..11]], [11, 11]);
        assert!(folds.is_empty());
        // Different length: only the long side gets a fold
        let folds = build_folds([[0..1, 0..1], [10..11, 20..21]], [11, 21]);
        assert_eq!(folds, [[None, Some(5..16)]]);
    }

    #[test]
    fn equal_files_fold_completely() {
        let d = diff(&lines("a", 30), &lines("a", 30));
        let f = Folding::new(&d);
        assert_eq!(f.folds().len(), 1);
        assert_eq!(f.folds()[0].lines, both(0..31));
        assert_eq!(f.layout(L, 10.).visual_line_count(), 1);
    }

    #[test]
    fn context_off_gives_no_folds() {
        let d = three_hunks();
        let f = Folding::none(&d);
        assert!(f.folds().is_empty());
        assert_eq!(f.layout(L, 10.).visual_line_count(), 101);
        assert_eq!(f.connectors().count(), 0);
    }

    #[test]
    fn opening_a_fold_shows_the_whole_run() {
        let d = three_hunks();
        let mut f = Folding::new(&d);
        let id = f.fold_at_line(L, 45).unwrap();
        assert_eq!(id, 2);
        assert_eq!(f.fold_at_line(R, 45), Some(id));
        assert_eq!(f.fold_at_line(L, 46), None);

        assert!(f.set_expanded(id, true));
        // No smaller fold is left inside 45..86, and both sides follow
        assert_eq!(visible(&f, L), [0..6, 15..36, 95..101]);
        assert_eq!(visible(&f, R), visible(&f, L));
        assert!((45..86).all(|line| f.fold_at_line(L, line).is_none()));
        // The folds 0..6, 15..36 and 95..101 each still take one visual line
        assert_eq!(f.layout(L, 10.).visual_line_count(), 101 - 5 - 20 - 5);
    }

    #[test]
    fn expand_and_collapse_all() {
        let d = three_hunks();
        let mut f = Folding::new(&d);
        assert!(f.expand_all(true));
        assert_eq!(f.visible(L).count(), 0);
        assert_eq!(f.layout(R, 10.).visual_line_count(), 101);
        assert!(!f.expand_all(true));
        f.expand_all(false);
        assert_eq!(f.visible(L).count(), 4);
    }

    #[test]
    fn expand_at_opens_the_fold_containing_the_line() {
        let d = three_hunks();
        let mut f = Folding::new(&d);
        // Inside the fold 45..86, not on its first line
        assert!(f.expand_at(L, 60));
        assert_eq!(visible(&f, L), [0..6, 15..36, 95..101]);
        assert!(!f.expand_at(L, 60));
        // On the first line
        assert!(f.expand_at(R, 0));
        assert_eq!(visible(&f, L), [15..36, 95..101]);
        // Inside a hunk there is nothing
        assert!(!f.expand_at(L, 10));
    }

    #[test]
    fn caret_in_collapsed_fold_moves_to_start() {
        let d = three_hunks();
        let f = Folding::new(&d);
        assert_eq!(f.caret_after(L, 50), 45);
        assert_eq!(f.caret_after(L, 45), 45);
        assert_eq!(f.caret_after(L, 40), 40);
        assert_eq!(f.caret_after(L, 3), 0);
    }

    #[test]
    fn fold_links_need_fold_on_both_sides() {
        // Right has 20 extra lines after line 10, so the last fold starts at 14 and 34
        let a = lines("a", 30);
        let b = format!(
            "{}{}{}",
            lines("a", 10),
            lines("x", 20),
            &a[lines("a", 10).len()..]
        );
        let d = diff(&a, &b);
        assert_eq!(d.hunks()[0].lines, [10..10, 10..30]);
        let mut f = Folding::new(&d);
        let c: Vec<_> = f.connectors().collect();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].lines, [0, 0]);
        assert_eq!(c[1].lines, [14, 34]);
        f.set_hovered(Some(c[1].id));
        assert!(f.connectors().nth(1).unwrap().hovered);

        // Only one side has a fold: no fold link
        let one_side = FoldBlock {
            lines: [None, Some(5..16)],
            expanded: false,
        };
        let f = Folding::with_folds([11, 21], vec![one_side]);
        assert_eq!(f.fold_at_line(R, 5), Some(0));
        assert_eq!(f.connectors().count(), 0);
    }

    #[test]
    fn hover_resets_when_fold_disappears() {
        let d = three_hunks();
        let mut f = Folding::new(&d);
        let id = f.fold_at_line(L, 45).unwrap();
        assert!(f.set_hovered(Some(id)));
        assert!(!f.set_hovered(Some(id)));
        f.set_expanded(id, true);
        assert_eq!(f.hovered(), None);
    }
}
