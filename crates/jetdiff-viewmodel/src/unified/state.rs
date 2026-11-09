//! The cursor, folding and scroll in Unified, like `DiffState` with one side. The cursor is a row,
//! and scrolloff counts visual lines, so a closed fold is one line.

use std::rc::Rc;

use jetdiff_core::side::Side;

use super::{RowLayout, UnifiedDiff};
use crate::columns;
use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::{UnifiedFolding, split_folding, unified_folding};
use crate::layout::{LineLayout, Viewport};
use crate::model::FileDiff;
use crate::nav::target_offsets;
use crate::scroll::{CodeViewKeeper, ScrollState};
use crate::scrolloff;
use crate::state::{DiffState, Horizontal, Outcome};

/// `h` and `l` scroll this many characters, as in Split.
const SCROLL_COLUMNS: f32 = 4.;

#[derive(Debug, Clone)]
pub struct UnifiedState {
    pub diff: Rc<UnifiedDiff>,
    pub folding: Rc<UnifiedFolding>,
    /// The cursor's row. Never inside a closed fold.
    pub row: usize,
    /// Both y are equal, so animation and drawing can share the type with Split.
    pub scroll: ScrollState,
}

impl UnifiedState {
    /// `fold_unchanged` closes the folds from the start, like `f`.
    pub fn new(diff: Rc<UnifiedDiff>, fold_unchanged: bool) -> Self {
        Self {
            folding: unified_folding(&diff, fold_unchanged),
            diff,
            row: 0,
            scroll: ScrollState::default(),
        }
    }

    /// From Split. The cursor's side and line are kept, and the line sits at the same height in
    /// the viewport. The folds follow `fold_unchanged`, not the folds opened in Split.
    pub fn from_split(split: &DiffState, fold_unchanged: bool, viewport: Viewport) -> Self {
        let cursor = split.cursor;
        let layout = split.folding.layout(cursor.side, viewport.line_height);
        let offset = layout.line_top(cursor.line) - split.scroll.y(cursor.side);
        let diff = Rc::new(UnifiedDiff::new(split.diff.clone()));
        let mut state = Self::new(diff, fold_unchanged);
        state.place(cursor, offset, split.scroll.x, viewport);
        state
    }

    /// Switches row layout the same way as toward Split.
    pub fn realign(&self, row_layout: RowLayout, fold_unchanged: bool, viewport: Viewport) -> Self {
        let diff = Rc::new(UnifiedDiff::with_layout(self.diff.diff.clone(), row_layout));
        let mut state = Self::new(diff, fold_unchanged);
        state.place(
            self.cursor(),
            self.cursor_offset(viewport),
            self.scroll.x,
            viewport,
        );
        state
    }

    /// To Split. Deleted rows give Left, everything else Right.
    pub fn to_split(&self, fold_unchanged: bool, viewport: Viewport) -> DiffState {
        let file_diff = self.diff.diff.clone();
        let folding = split_folding(&file_diff, fold_unchanged);
        let mut state = DiffState::new(file_diff, folding);
        state.scroll.x = self.scroll.x;
        state.place_cursor(self.cursor(), self.cursor_offset(viewport), viewport);
        state
    }

    /// The cursor's side and line.
    fn cursor(&self) -> Cursor {
        let row = self.diff.row(self.row);
        Cursor::new(row.side(), row.line())
    }

    /// How far below the top of the viewport the cursor's row is.
    fn cursor_offset(&self, viewport: Viewport) -> f32 {
        self.layout(viewport).line_top(self.row) - self.scroll_y()
    }

    /// The line sits `offset` below the top of the viewport, or the fold it is in if that is closed.
    fn place(&mut self, cursor: Cursor, offset: f32, scroll_x: f32, viewport: Viewport) {
        self.row = self.clamp_row(self.diff.row_of(cursor.side, cursor.line));
        self.scroll.x = scroll_x;
        let y = self.layout(viewport).line_top(self.row) - offset;
        self.set_scroll_y(y, viewport);
        self.follow(viewport, false);
    }

    /// The first hunk, like `DiffState::init`.
    pub fn init(&mut self, viewport: Viewport) {
        if self.diff.hunk_rows().is_empty() {
            return;
        }
        let (row, y) = self.target_for_hunk(0, viewport);
        self.row = row;
        self.scroll = ScrollState { y: [y, y], x: 0. };
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll.y[0]
    }

    fn layout(&self, viewport: Viewport) -> LineLayout<'_> {
        self.folding.layout(viewport.line_height)
    }

    fn last_row(&self) -> usize {
        self.diff.row_count() - 1
    }

    /// A valid row outside closed folds.
    fn clamp_row(&self, row: usize) -> usize {
        self.folding.cursor_after(row.min(self.last_row()))
    }

    fn cursor_visual(&self, viewport: Viewport) -> usize {
        self.layout(viewport).logical_to_visual(self.row)
    }

    fn set_visual(&mut self, visual: usize, viewport: Viewport) {
        let layout = self.layout(viewport);
        let visual = visual.min(layout.visual_line_count() - 1);
        self.row = self.clamp_row(layout.visual_to_logical(visual));
    }

    fn set_scroll_y(&mut self, y: f32, viewport: Viewport) {
        let y = self.layout(viewport).clamp_scroll(y, viewport.height);
        self.scroll.y = [y, y];
    }

    fn follow_y(&self, scroll_y: f32, viewport: Viewport, allow_center: bool) -> f32 {
        let layout = self.layout(viewport);
        let cursor = self.cursor_visual(viewport);
        scrolloff::follow_y(&layout, cursor, scroll_y, viewport, allow_center)
    }

    fn follow(&mut self, viewport: Viewport, allow_center: bool) {
        let y = self.follow_y(self.scroll_y(), viewport, allow_center);
        self.set_scroll_y(y, viewport);
    }

    /// After scroll without the cursor: move the cursor in if scrolloff requires it.
    fn push_cursor(&mut self, viewport: Viewport) -> bool {
        let layout = self.layout(viewport);
        let (min, max) = scrolloff::cursor_range(&layout, self.scroll_y(), viewport);
        let visual = self.cursor_visual(viewport);
        let target = visual.clamp(min, max);
        if target == visual {
            return false;
        }
        self.set_visual(target, viewport);
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
            MoveDown => self.move_visual(1, viewport),
            MoveUp => self.move_visual(-1, viewport),
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
            FoldOpen => self.change_folding(viewport, |row, folding| folding.open_at(row)),
            ToggleFolds => self.change_folding(viewport, |_, folding| {
                let any_closed = folding.any_closed();
                folding.set_all_open(any_closed)
            }),
            // One side
            FocusSide(_) => Outcome::default(),
            // Owned by the UI, as in Split
            CopyCursorLocation | Rollback | ZoomIn | ZoomOut | ResetZoom => Outcome::default(),
        }
    }

    fn move_visual(&mut self, delta: isize, viewport: Viewport) -> Outcome {
        let before = self.row;
        let visual = self.cursor_visual(viewport).saturating_add_signed(delta);
        self.set_visual(visual, viewport);
        if self.row == before {
            return Outcome::default();
        }
        self.follow(viewport, true);
        Outcome::instant()
    }

    fn go_to_row(&mut self, row: usize, viewport: Viewport) -> Outcome {
        self.row = self.clamp_row(row);
        self.follow(viewport, true);
        Outcome::animated()
    }

    /// Scroll and cursor move the same number of lines, like `DiffState`.
    fn half_page(&mut self, direction: isize, viewport: Viewport) -> Outcome {
        let lines = (scrolloff::screen_lines(viewport) / 2).max(1);
        let scroll_y = self.scroll_y();
        let visual = self.cursor_visual(viewport);
        self.set_visual(
            visual.saturating_add_signed(direction * lines as isize),
            viewport,
        );
        let moved = scroll_y + direction as f32 * lines as f32 * viewport.line_height;
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

    /// The end of the line, one character in from the right edge. A closed fold has no text.
    fn scroll_to_end(&mut self, horizontal: Horizontal) -> Outcome {
        let cells = if self.folding.fold_at_row(self.row).is_some() {
            0
        } else {
            columns::display_width(self.diff.text(self.row))
        };
        let x = (cells + 1) as f32 * horizontal.char_width - horizontal.text_width;
        self.scroll_x(x.max(0.), horizontal)
    }

    /// The same landing as Split: the start of the hunk about 1/3 down.
    fn target_for_hunk(&self, hunk: usize, viewport: Viewport) -> (usize, f32) {
        let rows = self.diff.hunk_rows()[hunk].clone();
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
        let hunks = self.diff.hunk_rows();
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

    /// The landing from `n` and `p`, for a hunk picked some other way. Hunks are never in a fold.
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

    /// `runBatchFoldingOperation` with `PositionKeeper`, on the one side.
    fn change_folding(
        &mut self,
        viewport: Viewport,
        operation: impl FnOnce(usize, &mut UnifiedFolding) -> bool,
    ) -> Outcome {
        let keeper =
            CodeViewKeeper::save(&self.layout(viewport), self.scroll_y(), self.row, viewport);
        if !operation(self.row, Rc::make_mut(&mut self.folding)) {
            return Outcome::default();
        }
        self.row = self.folding.cursor_after(self.row);
        let y = keeper.restore(&self.layout(viewport), self.row, viewport);
        self.scroll.y = [y, y];
        Outcome {
            folding_changed: true,
            ..Outcome::instant()
        }
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
    /// far as possible. The folds start over from `fold_unchanged`, as in Split.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, fold_unchanged: bool, viewport: Viewport) {
        let cursor = self.cursor();
        let unified = Rc::new(UnifiedDiff::with_layout(diff, self.diff.row_layout()));
        self.folding = unified_folding(&unified, fold_unchanged);
        self.diff = unified;
        self.row = self.clamp_row(self.diff.row_of(cursor.side, cursor.line));
        self.set_scroll_y(self.scroll_y(), viewport);
        self.follow(viewport, false);
    }

    /// Mouse wheel or trackpad. The cursor is only moved when scrolloff requires it.
    pub fn wheel(&mut self, delta_y: f32, viewport: Viewport) -> bool {
        self.set_scroll_y(self.scroll_y() + delta_y, viewport);
        self.push_cursor(viewport)
    }

    /// A click selects the row without scrolling. A row in a closed fold gives the fold.
    pub fn click(&mut self, row: usize) {
        self.row = self.clamp_row(row);
    }

    pub fn resize(&mut self, viewport: Viewport) {
        self.set_scroll_y(self.scroll_y(), viewport);
        self.push_cursor(viewport);
    }

    /// Zoom: the cursor row is kept at the same place in the viewport, measured in lines.
    pub fn rescale(&mut self, previous_line_height: f32, viewport: Viewport, x_scale: f32) {
        let visual = self.cursor_visual(viewport) as f32;
        let offset = visual - self.scroll_y() / previous_line_height;
        self.set_scroll_y((visual - offset) * viewport.line_height, viewport);
        self.scroll.x *= x_scale;
        self.push_cursor(viewport);
    }
}
