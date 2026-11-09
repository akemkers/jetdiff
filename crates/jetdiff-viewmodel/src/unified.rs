//! Unified diff: one code view where deleted lines sit above the added ones in each hunk. Port of
//! `UnifiedFragmentBuilder`, but the rows point into `FileDiff` instead of building new
//! text, so syntax and inline highlights are reused. `RowLayout::Split` gives the same rows with
//! the deleted and added line next to each other, for Split in the terminal.

use std::ops::Range;
use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::folding::UnifiedFolding;
use crate::hunk_minimap::{HunkMinimapMark, hunk_minimap_mark_y};
use crate::model::{FileDiff, Hunk, HunkKind, idx};

mod excerpt;
mod state;

pub use state::UnifiedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// The line on each side. Deleted is `[Some, None]`, added is `[None, Some]` and unchanged has
    /// both. Added and deleted files have only one side.
    pub lines: [Option<usize>; 2],
    pub hunk: Option<usize>,
}

impl Row {
    /// The side the text is taken from. Unchanged lines are taken from Right, like `UnifiedFragmentBuilder`.
    pub fn side(&self) -> Side {
        if self.lines[1].is_some() {
            Side::Right
        } else {
            Side::Left
        }
    }

    pub fn line(&self) -> usize {
        self.lines[idx(self.side())].unwrap_or(0)
    }

    /// Deleted rows get the deleted color and added rows the added color, also at word level. Unified has
    /// no `Modified`.
    pub fn kind(&self) -> HunkKind {
        match self.side() {
            Side::Left => HunkKind::Deleted,
            Side::Right => HunkKind::Added,
        }
    }
}

/// How the lines in a hunk are placed in the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowLayout {
    /// Deleted lines above the added ones.
    Unified,
    /// Deleted and added line next to each other, for Aligned Split. The shorter side gets rows
    /// without a line.
    Split,
}

#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    pub diff: Rc<FileDiff>,
    rows: Vec<Row>,
    /// The row each line is on, per side. Unchanged lines on Left point at the same
    /// row as the line on Right.
    row_of: [Vec<usize>; 2],
    /// The rows of each hunk.
    hunk_rows: Vec<Range<usize>>,
    row_layout: RowLayout,
}

impl UnifiedDiff {
    pub fn new(diff: Rc<FileDiff>) -> Self {
        Self::with_layout(diff, RowLayout::Unified)
    }

    pub fn row_layout(&self) -> RowLayout {
        self.row_layout
    }

    pub fn with_layout(diff: Rc<FileDiff>, row_layout: RowLayout) -> Self {
        let line_counts = [diff.line_count(Side::Left), diff.line_count(Side::Right)];
        let mut rows = Vec::with_capacity(line_counts[0] + line_counts[1]);
        let mut row_of = line_counts.map(|count| vec![0; count]);
        let mut hunk_rows = Vec::with_capacity(diff.hunks().len());
        let mut push = |lines: [Option<usize>; 2], hunk| {
            for side in [Side::Left, Side::Right] {
                if let Some(line) = lines[idx(side)] {
                    row_of[idx(side)][line] = rows.len();
                }
            }
            rows.push(Row { lines, hunk });
            rows.len()
        };

        let mut next = [0, 0];
        let mut len = 0;
        for (index, hunk) in diff.hunks().iter().enumerate() {
            let [left_side, right_side] = &hunk.lines;
            for (left_side_line, right_side_line) in
                (next[0]..left_side.start).zip(next[1]..right_side.start)
            {
                len = push([Some(left_side_line), Some(right_side_line)], None);
            }
            let start = len;
            match row_layout {
                RowLayout::Unified => {
                    for line in left_side.clone() {
                        len = push([Some(line), None], Some(index));
                    }
                    for line in right_side.clone() {
                        len = push([None, Some(line)], Some(index));
                    }
                }
                RowLayout::Split => {
                    for lines in pair_lines(&diff, hunk) {
                        len = push(lines, Some(index));
                    }
                }
            }
            hunk_rows.push(start..len);
            next = [left_side.end, right_side.end];
        }
        for (left_side_line, right_side_line) in
            (next[0]..line_counts[0]).zip(next[1]..line_counts[1])
        {
            push([Some(left_side_line), Some(right_side_line)], None);
        }

        // The empty side has one empty line, and it should not have a number
        if let Some(shown) = diff.single_side() {
            for row in &mut rows {
                row.lines[idx(shown.other())] = None;
            }
        }
        Self {
            diff,
            rows,
            row_of,
            hunk_rows,
            row_layout,
        }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn row(&self, row: usize) -> &Row {
        &self.rows[row]
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn row_of(&self, side: Side, line: usize) -> usize {
        let lines = &self.row_of[idx(side)];
        lines[line.min(lines.len() - 1)]
    }

    pub fn hunk_rows(&self) -> &[Range<usize>] {
        &self.hunk_rows
    }

    pub fn text(&self, row: usize) -> &str {
        let row = self.row(row);
        self.diff.line(row.side(), row.line())
    }

    /// Two hunk minimap marks for a Modified hunk: one for the deleted part and one for the added part. Split has one per
    /// hunk. Y follows the visual lines of `folding`.
    pub fn hunk_minimap_marks(
        &self,
        folding: &UnifiedFolding,
        line_height: f32,
    ) -> Vec<HunkMinimapMark> {
        let layout = folding.layout(line_height);
        let mut marks = Vec::new();
        for (rows, hunk) in self.hunk_rows.iter().zip(self.diff.hunks()) {
            if self.row_layout == RowLayout::Split {
                let (start_y, end_y) = hunk_minimap_mark_y(&layout, rows.clone());
                marks.push(HunkMinimapMark {
                    kind: hunk.kind,
                    start_y,
                    end_y,
                });
                continue;
            }
            let deleted = self.rows[rows.clone()]
                .iter()
                .take_while(|row| row.side() == Side::Left)
                .count();
            let first_added = rows.start + deleted;
            for (part, kind) in [
                (rows.start..first_added, HunkKind::Deleted),
                (first_added..rows.end, HunkKind::Added),
            ] {
                if part.is_empty() {
                    continue;
                }
                let (start_y, end_y) = hunk_minimap_mark_y(&layout, part);
                marks.push(HunkMinimapMark {
                    kind,
                    start_y,
                    end_y,
                });
            }
        }
        marks
    }
}

/// Lines in a hunk that have unchanged text in common sit on the same row. The rest are paired in
/// order, and the shorter side gets rows without a line.
fn pair_lines(diff: &FileDiff, hunk: &Hunk) -> Vec<[Option<usize>; 2]> {
    let [left_side, right_side] = &hunk.lines;
    let mut rows = Vec::new();
    let zip = |left_side_lines: Range<usize>,
               right_side_lines: Range<usize>,
               rows: &mut Vec<[Option<usize>; 2]>| {
        let at = |lines: &Range<usize>, i: usize| (i < lines.len()).then(|| lines.start + i);
        for i in 0..left_side_lines.len().max(right_side_lines.len()) {
            rows.push([at(&left_side_lines, i), at(&right_side_lines, i)]);
        }
    };
    let mut anchors: Vec<(usize, usize)> = Vec::new();
    if let Some(inline_highlights) = &hunk.inline_highlights {
        let texts =
            [Side::Left, Side::Right].map(|side| &diff.text(side)[hunk.offsets[idx(side)].clone()]);
        let ends = [texts[0].len(), texts[1].len()];
        let highlights = inline_highlights
            .iter()
            .map(|highlight| [highlight.offsets1.clone(), highlight.offsets2.clone()])
            .chain([[ends[0]..ends[0], ends[1]..ends[1]]]);
        let mut equal_from = [0, 0];
        for [left_side_highlight, right_side_highlight] in highlights {
            let equal = (left_side_highlight.start - equal_from[0])
                .min(right_side_highlight.start - equal_from[1]);
            let segment = &texts[0][equal_from[0]..equal_from[0] + equal];
            for (offset, _) in segment.char_indices().filter(|(_, ch)| !ch.is_whitespace()) {
                let line = |side: Side| {
                    let from = hunk.offsets[idx(side)].start + equal_from[idx(side)];
                    diff.line_of_offset(side, from + offset)
                };
                let pair = (line(Side::Left), line(Side::Right));
                if anchors
                    .last()
                    .is_none_or(|&(last_left_side, last_right_side)| {
                        pair.0 > last_left_side && pair.1 > last_right_side
                    })
                {
                    anchors.push(pair);
                }
            }
            equal_from = [left_side_highlight.end, right_side_highlight.end];
        }
    }
    let (mut next_left_side, mut next_right_side) = (left_side.start, right_side.start);
    for (anchor_left_side, anchor_right_side) in anchors {
        zip(
            next_left_side..anchor_left_side,
            next_right_side..anchor_right_side,
            &mut rows,
        );
        rows.push([Some(anchor_left_side), Some(anchor_right_side)]);
        (next_left_side, next_right_side) = (anchor_left_side + 1, anchor_right_side + 1);
    }
    zip(
        next_left_side..left_side.end,
        next_right_side..right_side.end,
        &mut rows,
    );
    rows
}

#[cfg(test)]
mod tests;
