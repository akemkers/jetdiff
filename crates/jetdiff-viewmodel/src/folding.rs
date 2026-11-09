//! Folding of unchanged lines. Port of `FoldingModelSupport`, where `FoldedBlock` holds the regions
//! paired on each side. IntelliJ nests three folds per run of unchanged lines, jetdiff builds one
//! (ADR-0002). Split has two sides, and Unified has one side over its rows, like
//! `UnifiedFoldingModel`.

use std::ops::Range;
use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::layout::{LineLayout, VisualFold, visual_folds};
use crate::model::{FileDiff, idx};
use crate::unified::UnifiedDiff;

/// Lines shown around each hunk, like the default in `TextDiffSettings.contextRange`.
const CONTEXT_RANGE: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold<const SIDES: usize = 2> {
    /// `None` when the side has fewer than two lines to fold.
    pub lines: [Option<Range<usize>>; SIDES],
    /// Shared by the sides, since `MyFoldingListener` mirrors the state.
    pub open: bool,
}

impl Fold {
    pub fn lines(&self, side: Side) -> Option<Range<usize>> {
        self.lines[idx(side)].clone()
    }
}

/// Index into `Folding::folds`.
pub type FoldId = usize;

/// A closed fold with lines on both sides, so it has a fold link. With an id for click and hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedFold {
    pub id: FoldId,
    /// The first line of the fold per side.
    pub lines: [usize; 2],
    pub hovered: bool,
}

/// The folds of Unified, over its rows.
pub type UnifiedFolding = Folding<1>;

#[derive(Debug, Clone)]
pub struct Folding<const SIDES: usize = 2> {
    line_counts: [usize; SIDES],
    /// One per run of unchanged lines, in line order.
    folds: Vec<Fold<SIDES>>,
    visible: [Vec<VisualFold>; SIDES],
    visible_ids: [Vec<FoldId>; SIDES],
    hovered: Option<FoldId>,
}

impl Folding {
    pub fn new(diff: &FileDiff) -> Self {
        let hunks = diff.hunks().iter().map(|hunk| hunk.lines.clone());
        Self::build(hunks, line_counts(diff))
    }

    /// No folds. For tests and excerpts.
    pub fn none(diff: &FileDiff) -> Self {
        Self::with_folds(line_counts(diff), Vec::new())
    }

    pub fn layout(&self, side: Side, line_height: f32) -> LineLayout<'_> {
        self.layout_of(idx(side), line_height)
    }

    pub fn layouts(&self, line_height: f32) -> [LineLayout<'_>; 2] {
        [
            self.layout(Side::Left, line_height),
            self.layout(Side::Right, line_height),
        ]
    }

    /// Closed folds shown on the side, in order.
    pub fn visible(&self, side: Side) -> impl Iterator<Item = (FoldId, &VisualFold)> {
        self.visible_of(idx(side))
    }

    /// `getBlockForLine`: the closed fold that starts on the line. Other lines in a fold are
    /// hidden, and `yPositionToLogicalLine` always gives the first line.
    pub fn fold_at_line(&self, side: Side, line: usize) -> Option<FoldId> {
        self.fold_starting_at(idx(side), line)
    }

    /// `ExpandRegionAction` at the cursor: the closed fold that contains the line.
    pub fn open_at(&mut self, side: Side, line: usize) -> bool {
        self.open_containing(idx(side), line)
    }

    /// `FoldingModelImpl.onFoldProcessingEnd`: a cursor inside a closed fold is moved to
    /// its start.
    pub fn cursor_after(&self, side: Side, line: usize) -> usize {
        self.out_of_closed_fold(idx(side), line)
    }

    /// `MyPaintable.process`: the folds that are closed on both sides. If one side has no fold,
    /// there is no fold link.
    pub fn closed_folds(&self) -> impl Iterator<Item = ClosedFold> + '_ {
        self.folds.iter().enumerate().filter_map(|(id, fold)| {
            let [Some(left_side_lines), Some(right_side_lines)] = &fold.lines else {
                return None;
            };
            (!fold.open).then_some(ClosedFold {
                id,
                lines: [left_side_lines.start, right_side_lines.start],
                hovered: self.hovered == Some(id),
            })
        })
    }
}

/// One side, where the rows are the lines and each hunk's rows are the change. Unchanged rows are
/// 1:1 with the unchanged lines, so the folds are on the same lines as in Split.
impl UnifiedFolding {
    pub fn unified(diff: &UnifiedDiff) -> Self {
        let hunks = diff.hunk_rows().iter().map(|rows| [rows.clone()]);
        Self::build(hunks, [diff.row_count()])
    }

    /// No folds. For excerpts.
    pub fn unified_none(diff: &UnifiedDiff) -> Self {
        Self::with_folds([diff.row_count()], Vec::new())
    }

    pub fn layout(&self, line_height: f32) -> LineLayout<'_> {
        self.layout_of(0, line_height)
    }

    pub fn visible(&self) -> impl Iterator<Item = (FoldId, &VisualFold)> {
        self.visible_of(0)
    }

    pub fn fold_at_row(&self, row: usize) -> Option<FoldId> {
        self.fold_starting_at(0, row)
    }

    pub fn open_at(&mut self, row: usize) -> bool {
        self.open_containing(0, row)
    }

    pub fn cursor_after(&self, row: usize) -> usize {
        self.out_of_closed_fold(0, row)
    }
}

impl<const SIDES: usize> Folding<SIDES> {
    fn build(
        hunks: impl IntoIterator<Item = [Range<usize>; SIDES]>,
        line_counts: [usize; SIDES],
    ) -> Self {
        let folds = build_folds(hunks, line_counts)
            .into_iter()
            .map(|lines| Fold { lines, open: false })
            .collect();
        Self::with_folds(line_counts, folds)
    }

    fn with_folds(line_counts: [usize; SIDES], folds: Vec<Fold<SIDES>>) -> Self {
        let mut folding = Self {
            line_counts,
            folds,
            visible: std::array::from_fn(|_| Vec::new()),
            visible_ids: std::array::from_fn(|_| Vec::new()),
            hovered: None,
        };
        folding.update();
        folding
    }

    /// The folds as a file starts with them: closed if `fold_unchanged`, like `f`.
    fn starting(mut self, fold_unchanged: bool) -> Rc<Self> {
        self.set_all_open(!fold_unchanged);
        Rc::new(self)
    }

    pub fn folds(&self) -> &[Fold<SIDES>] {
        &self.folds
    }

    pub fn any_closed(&self) -> bool {
        self.folds.iter().any(|fold| !fold.open)
    }

    /// `FoldedBlock.setExpanded`: all sides.
    pub fn set_open(&mut self, id: FoldId, open: bool) -> bool {
        let fold = &mut self.folds[id];
        if fold.open == open {
            return false;
        }
        fold.open = open;
        self.update();
        true
    }

    /// `FoldingModelSupport.expandAll`.
    pub fn set_all_open(&mut self, open: bool) -> bool {
        let mut changed = false;
        for fold in &mut self.folds {
            changed |= fold.open != open;
            fold.open = open;
        }
        self.update();
        changed
    }

    pub fn hovered(&self) -> Option<FoldId> {
        self.hovered
    }

    pub fn set_hovered(&mut self, id: Option<FoldId>) -> bool {
        let changed = self.hovered != id;
        self.hovered = id;
        changed
    }

    fn layout_of(&self, side: usize, line_height: f32) -> LineLayout<'_> {
        LineLayout::with_folds(self.line_counts[side], line_height, &self.visible[side])
    }

    fn visible_of(&self, side: usize) -> impl Iterator<Item = (FoldId, &VisualFold)> {
        self.visible_ids[side]
            .iter()
            .copied()
            .zip(&self.visible[side])
    }

    fn fold_starting_at(&self, side: usize, line: usize) -> Option<FoldId> {
        let folds = &self.visible[side];
        let k = folds.partition_point(|f| f.lines.start < line);
        folds
            .get(k)
            .filter(|f| f.lines.start == line)
            .map(|_| self.visible_ids[side][k])
    }

    fn open_containing(&mut self, side: usize, line: usize) -> bool {
        let found = self.folds.iter().position(|fold| {
            !fold.open
                && fold.lines[side]
                    .as_ref()
                    .is_some_and(|lines| lines.contains(&line))
        });
        found.is_some_and(|id| self.set_open(id, true))
    }

    fn out_of_closed_fold(&self, side: usize, line: usize) -> usize {
        let folds = &self.visible[side];
        let k = folds.partition_point(|f| f.lines.start <= line);
        match k.checked_sub(1).map(|k| &folds[k]) {
            Some(f) if line < f.lines.end => f.lines.start,
            _ => line,
        }
    }

    /// The closed folds per side, which `FoldingModel` shows with a fold wave
    /// (`getHighlighterCondition`).
    fn update(&mut self) {
        for side in 0..SIDES {
            let (ids, ranges): (Vec<_>, Vec<_>) = self
                .folds
                .iter()
                .enumerate()
                .filter(|(_, fold)| !fold.open)
                .filter_map(|(id, fold)| Some((id, fold.lines[side].clone()?)))
                .unzip();
            self.visible[side] = visual_folds(ranges);
            self.visible_ids[side] = ids;
        }
        if self
            .hovered
            .is_some_and(|id| !self.visible_ids.iter().flatten().any(|&v| v == id))
        {
            self.hovered = None;
        }
    }
}

/// The folding a file starts with in Split.
pub(crate) fn split_folding(diff: &FileDiff, fold_unchanged: bool) -> Rc<Folding> {
    Folding::new(diff).starting(fold_unchanged)
}

/// The folding a file starts with in Unified.
pub(crate) fn unified_folding(diff: &UnifiedDiff, fold_unchanged: bool) -> Rc<UnifiedFolding> {
    UnifiedFolding::unified(diff).starting(fold_unchanged)
}

fn line_counts(diff: &FileDiff) -> [usize; 2] {
    [diff.line_count(Side::Left), diff.line_count(Side::Right)]
}

/// `FoldingBuilderBase.build` and `addGroup` with only the first level: one fold per run of
/// unchanged lines between the hunks, plus before the first and after the last.
fn build_folds<const SIDES: usize>(
    hunks: impl IntoIterator<Item = [Range<usize>; SIDES]>,
    line_counts: [usize; SIDES],
) -> Vec<[Option<Range<usize>>; SIDES]> {
    let mut folds = Vec::new();
    let counts = line_counts.map(|c| c as i64);
    let context = CONTEXT_RANGE as i64;
    let mut add = |starts: [i64; SIDES], ends: [i64; SIDES]| {
        let regions: [Option<Range<usize>>; SIDES] = std::array::from_fn(|i| {
            let start = starts[i].saturating_add(context).clamp(0, counts[i]);
            let end = ends[i].saturating_sub(context).clamp(0, counts[i]);
            (end - start >= 2).then_some(start as usize..end as usize)
        });
        if regions.iter().any(Option::is_some) {
            folds.push(regions);
        }
    };
    // `Integer.MIN_VALUE` and `MAX_VALUE`: the first and last fold go all the way to the edge
    let mut last = [i64::MIN; SIDES];
    for lines in hunks {
        let starts = last;
        last = lines.clone().map(|lines| lines.end as i64);
        add(starts, lines.map(|lines| lines.start as i64));
    }
    add(last, [i64::MAX; SIDES]);
    folds
}

#[cfg(test)]
mod tests;
