//! Unified diff: one code view where deleted lines sit above the added ones in each hunk. Port of
//! `UnifiedFragmentBuilder`, but the rows point into `FileDiff` instead of building new
//! text, so syntax and inline highlights are reused. No folding. `RowLayout::Split` gives the same
//! rows with the deleted and added line next to each other, for Split in the terminal.

use std::ops::Range;
use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::columns;
use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::hunk_minimap::{HunkMinimapMark, hunk_minimap_mark_y};
use crate::layout::{LineLayout, Viewport};
use crate::model::{FileDiff, Hunk, HunkKind, idx};
use crate::nav::target_offsets;
use crate::scroll::ScrollState;
use crate::scrolloff;
use crate::state::{DiffState, Horizontal, Outcome};

/// `h` and `l` scroll this many characters, as in Split.
const SCROLL_COLUMNS: f32 = 4.;

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
    /// Deleted and added line next to each other, for Split in the terminal. The shorter side gets rows
    /// without a line. Split with the diff divider in GPUI is `DiffState`, not this.
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

    /// The rows of `hunks` with `context` unchanged rows around each, in order. Excerpts that
    /// overlap or touch become one.
    pub fn excerpts(&self, hunks: &[usize], context: usize) -> Vec<Range<usize>> {
        let mut rows: Vec<Range<usize>> = hunks
            .iter()
            .map(|&hunk| {
                let rows = &self.hunk_rows[hunk];
                rows.start.saturating_sub(context)..(rows.end + context).min(self.rows.len())
            })
            .collect();
        rows.sort_by_key(|rows| rows.start);
        let mut excerpts: Vec<Range<usize>> = Vec::with_capacity(rows.len());
        for rows in rows {
            match excerpts.last_mut() {
                Some(last) if rows.start <= last.end => last.end = last.end.max(rows.end),
                _ => excerpts.push(rows),
            }
        }
        excerpts
    }

    pub fn text(&self, row: usize) -> &str {
        let row = self.row(row);
        self.diff.line(row.side(), row.line())
    }

    pub fn layout(&self, line_height: f32) -> LineLayout<'static> {
        LineLayout::new(self.row_count(), line_height)
    }

    /// Two hunk minimap marks for a Modified hunk: one for the deleted part and one for the added part. Split has one per
    /// hunk.
    pub fn hunk_minimap_marks(&self, line_height: f32) -> Vec<HunkMinimapMark> {
        let layout = self.layout(line_height);
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

#[derive(Debug, Clone)]
pub struct UnifiedState {
    pub diff: Rc<UnifiedDiff>,
    /// The cursor's row.
    pub row: usize,
    /// Both y are equal, so animation and drawing can share the type with Split.
    pub scroll: ScrollState,
}

impl UnifiedState {
    pub fn new(diff: Rc<UnifiedDiff>) -> Self {
        Self {
            diff,
            row: 0,
            scroll: ScrollState::default(),
        }
    }

    /// From Split. The cursor's side and line are kept, and the line sits at the same height in
    /// the viewport.
    pub fn from_split(split: &DiffState, viewport: Viewport) -> Self {
        let cursor = split.cursor;
        let layout = split.folding.layout(cursor.side, viewport.line_height);
        let offset = layout.line_top(cursor.line) - split.scroll.y(cursor.side);
        let diff = Rc::new(UnifiedDiff::new(split.diff.clone()));
        Self::place(diff, cursor, offset, split.scroll.x, viewport)
    }

    /// Switches row layout the same way as toward Split.
    pub fn realign(&self, row_layout: RowLayout, viewport: Viewport) -> Self {
        let row = self.diff.row(self.row);
        let cursor = Cursor::new(row.side(), row.line());
        let offset = self.row as f32 * viewport.line_height - self.scroll_y();
        let diff = Rc::new(UnifiedDiff::with_layout(self.diff.diff.clone(), row_layout));
        Self::place(diff, cursor, offset, self.scroll.x, viewport)
    }

    /// The line sits `offset` below the top of the viewport.
    fn place(
        diff: Rc<UnifiedDiff>,
        cursor: Cursor,
        offset: f32,
        scroll_x: f32,
        viewport: Viewport,
    ) -> Self {
        let mut state = Self::new(diff);
        state.row = state.diff.row_of(cursor.side, cursor.line);
        state.scroll.x = scroll_x;
        let y = state.row as f32 * viewport.line_height - offset;
        state.set_scroll_y(y, viewport);
        state.follow(viewport, false);
        state
    }

    /// To Split. Deleted rows give Left, everything else Right.
    pub fn to_split(&self, folding: Rc<Folding>, viewport: Viewport) -> DiffState {
        let row = self.diff.row(self.row);
        let offset = self.row as f32 * viewport.line_height - self.scroll_y();
        let mut state = DiffState::new(self.diff.diff.clone(), folding);
        state.scroll.x = self.scroll.x;
        let cursor = Cursor::new(row.side(), row.line());
        state.place_cursor(cursor, offset, viewport);
        state
    }

    /// The first hunk, like `DiffState::init`.
    pub fn init(&mut self, viewport: Viewport) {
        if self.diff.hunk_rows.is_empty() {
            return;
        }
        let (row, y) = self.target_for_hunk(0, viewport);
        self.row = row;
        self.scroll = ScrollState { y: [y, y], x: 0. };
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll.y[0]
    }

    fn layout(&self, viewport: Viewport) -> LineLayout<'static> {
        self.diff.layout(viewport.line_height)
    }

    fn last_row(&self) -> usize {
        self.diff.row_count() - 1
    }

    fn set_scroll_y(&mut self, y: f32, viewport: Viewport) {
        let y = self.layout(viewport).clamp_scroll(y, viewport.height);
        self.scroll.y = [y, y];
    }

    fn follow_y(&self, scroll_y: f32, viewport: Viewport, allow_center: bool) -> f32 {
        let layout = self.layout(viewport);
        scrolloff::follow_y(&layout, self.row, scroll_y, viewport, allow_center)
    }

    fn follow(&mut self, viewport: Viewport, allow_center: bool) {
        let y = self.follow_y(self.scroll_y(), viewport, allow_center);
        self.set_scroll_y(y, viewport);
    }

    /// After scroll without the cursor: move the cursor in if scrolloff requires it.
    fn push_cursor(&mut self, viewport: Viewport) -> bool {
        let layout = self.layout(viewport);
        let (min, max) = scrolloff::cursor_range(&layout, self.scroll_y(), viewport);
        let row = self.row.clamp(min, max);
        if row == self.row {
            return false;
        }
        self.row = row;
        true
    }

    pub fn execute(
        &mut self,
        command: Command,
        viewport: Viewport,
        horizontal: Horizontal,
    ) -> Outcome {
        use Command::*;
        let columns = SCROLL_COLUMNS * horizontal.char_width;
        match command {
            MoveDown => self.move_row(1, viewport),
            MoveUp => self.move_row(-1, viewport),
            ScrollLeft => self.scroll_x(self.scroll.x - columns, horizontal),
            ScrollRight => self.scroll_x(self.scroll.x + columns, horizontal),
            ScrollToStart => self.scroll_x(0., horizontal),
            ScrollToEnd => self.scroll_to_end(horizontal),
            GoToTop => self.go_to_row(0, viewport),
            GoToBottom => self.go_to_row(self.last_row(), viewport),
            HalfPageDown => self.half_page(1, viewport),
            HalfPageUp => self.half_page(-1, viewport),
            NextHunk => self.go_to_hunk(true, viewport),
            PreviousHunk => self.go_to_hunk(false, viewport),
            // One side and no folding
            FocusSide(_) | FoldOpen | ToggleFolds => Outcome::default(),
            // Owned by the UI, as in Split
            CopyCursorLocation | Rollback | ZoomIn | ZoomOut | ResetZoom => Outcome::default(),
        }
    }

    fn move_row(&mut self, delta: isize, viewport: Viewport) -> Outcome {
        let row = self.row.saturating_add_signed(delta).min(self.last_row());
        if row == self.row {
            return Outcome::default();
        }
        self.row = row;
        self.follow(viewport, true);
        Outcome::instant()
    }

    fn go_to_row(&mut self, row: usize, viewport: Viewport) -> Outcome {
        self.row = row;
        self.follow(viewport, true);
        Outcome::animated()
    }

    /// Scroll and cursor move the same number of rows, like `DiffState`.
    fn half_page(&mut self, direction: isize, viewport: Viewport) -> Outcome {
        let rows = (scrolloff::screen_lines(viewport) / 2).max(1);
        let scroll_y = self.scroll_y();
        self.row = self
            .row
            .saturating_add_signed(direction * rows as isize)
            .min(self.last_row());
        let moved = scroll_y + direction as f32 * rows as f32 * viewport.line_height;
        let y = self.follow_y(moved, viewport, false);
        self.set_scroll_y(y, viewport);
        Outcome::animated()
    }

    fn scroll_x(&mut self, x: f32, horizontal: Horizontal) -> Outcome {
        let before = self.scroll.x;
        self.scroll.scroll_x_to(x, horizontal.max_x);
        if self.scroll.x == before {
            return Outcome::default();
        }
        Outcome::instant()
    }

    /// The end of the line, one character in from the right edge.
    fn scroll_to_end(&mut self, horizontal: Horizontal) -> Outcome {
        let cells = columns::display_width(self.diff.text(self.row));
        let x = (cells + 1) as f32 * horizontal.char_width - horizontal.text_width;
        self.scroll_x(x.max(0.), horizontal)
    }

    /// The same landing as Split: the start of the hunk about 1/3 down.
    fn target_for_hunk(&self, hunk: usize, viewport: Viewport) -> (usize, f32) {
        let rows = self.diff.hunk_rows[hunk].clone();
        let layout = self.layout(viewport);
        let [y, _] = target_offsets(
            &[layout, layout],
            [rows.start; 2],
            [rows.end; 2],
            viewport.height,
        );
        (rows.start, y)
    }

    /// `n` goes to the first hunk below the cursor, `p` to the last hunk entirely above.
    fn go_to_hunk(&mut self, next: bool, viewport: Viewport) -> Outcome {
        let hunks = &self.diff.hunk_rows;
        let hunk = if next {
            hunks.iter().position(|rows| rows.start > self.row)
        } else {
            hunks.iter().rposition(|rows| rows.end <= self.row)
        };
        match hunk {
            Some(hunk) => self.land_on_hunk(hunk, viewport),
            None => Outcome::default(),
        }
    }

    /// The landing from `n` and `p`, for a hunk picked some other way.
    pub fn land_on_hunk(&mut self, hunk: usize, viewport: Viewport) -> Outcome {
        let (row, y) = self.target_for_hunk(hunk, viewport);
        self.row = row;
        self.scroll = ScrollState { y: [y, y], x: 0. };
        let followed = self.follow_y(y, viewport, false);
        if followed != y {
            self.set_scroll_y(followed, viewport);
        }
        Outcome::animated()
    }

    /// The hunk the cursor is in.
    pub fn cursor_hunk(&self) -> Option<usize> {
        self.diff.row(self.row).hunk
    }

    /// The cursor's line in the newest version, like `DiffState::cursor_location`. A deleted row gives the line
    /// right below on Right. Deleted files only have Left.
    pub fn cursor_location(&self) -> Cursor {
        let row = self.diff.row(self.row);
        let diff = &self.diff.diff;
        if row.side() == Side::Right || diff.single_side().is_some() {
            return Cursor::new(row.side(), row.line());
        }
        let line = diff.mapping().transfer(Side::Left, row.line());
        Cursor::new(Side::Right, line.min(diff.line_count(Side::Right) - 1))
    }

    /// The same file with new content. The cursor stays on the same line, and scroll is kept as
    /// far as possible.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, viewport: Viewport) {
        let row = *self.diff.row(self.row);
        self.diff = Rc::new(UnifiedDiff::with_layout(diff, self.diff.row_layout));
        self.row = self.diff.row_of(row.side(), row.line());
        self.set_scroll_y(self.scroll_y(), viewport);
        self.follow(viewport, false);
    }

    /// Mouse wheel or trackpad. The cursor is only moved when scrolloff requires it.
    pub fn wheel(&mut self, delta_y: f32, viewport: Viewport) -> bool {
        self.set_scroll_y(self.scroll_y() + delta_y, viewport);
        self.push_cursor(viewport)
    }

    /// A click selects the row without scrolling.
    pub fn click(&mut self, row: usize) {
        self.row = row.min(self.last_row());
    }

    pub fn resize(&mut self, viewport: Viewport) {
        self.set_scroll_y(self.scroll_y(), viewport);
        self.push_cursor(viewport);
    }

    /// Zoom: the cursor row is kept at the same place in the viewport, measured in rows.
    pub fn rescale(&mut self, previous_line_height: f32, viewport: Viewport, x_scale: f32) {
        let row = self.row as f32;
        let offset = row - self.scroll_y() / previous_line_height;
        self.set_scroll_y((row - offset) * viewport.line_height, viewport);
        self.scroll.x *= x_scale;
        self.push_cursor(viewport);
    }
}

#[cfg(test)]
mod tests;
