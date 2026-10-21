//! The diff as a whole: cursor, folding and scroll. Carries out `Command` and keeps
//! the line and scroll in step with scrolloff. `scroll` is the target; the UI animates there itself.

use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::columns;
use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::{FileDiff, idx};
use crate::nav::{next_block, prev_block, target_for_block};
use crate::scroll::{PositionKeeper, ScrollState};
use crate::scrolloff;

/// `h` and `l` scroll this many characters.
const SCROLL_COLUMNS: f32 = 4.;

/// Horizontal geometry from the UI, which knows the text width.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Horizontal {
    pub text_width: f32,
    pub char_width: f32,
    pub max_x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcome {
    pub changed: bool,
    /// Scroll should be animated to the target instead of jumping.
    pub animate: bool,
    pub folding_changed: bool,
}

impl Outcome {
    pub(crate) fn instant() -> Self {
        Self {
            changed: true,
            ..Self::default()
        }
    }

    pub(crate) fn animated() -> Self {
        Self {
            animate: true,
            ..Self::instant()
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffState {
    pub diff: Rc<FileDiff>,
    pub folding: Rc<Folding>,
    pub scroll: ScrollState,
    /// The side sync scroll is computed from. Follows the cursor, but the mouse wheel can take it.
    pub master: Side,
    pub cursor: Cursor,
}

impl DiffState {
    pub fn new(diff: Rc<FileDiff>, folding: Rc<Folding>) -> Self {
        // `getCurrentSide` is RIGHT by default in SimpleDiffViewer
        let side = Side::Right;
        Self {
            diff,
            folding,
            scroll: ScrollState::default(),
            master: side,
            cursor: Cursor::new(side, 0),
        }
    }

    /// `InitialScrollPositionSupport`: the first hunk, with the cursor on Right.
    pub fn init(&mut self, vp: Viewport) {
        if self.diff.blocks().is_empty() {
            return;
        }
        let t = target_for_block(&self.diff, &self.folding, 0, vp);
        self.scroll.y = t.scroll_y;
        self.scroll.x = t.scroll_x;
        let side = self.cursor.side;
        self.cursor = Cursor::new(side, self.clamp_line(side, t.caret_lines[idx(side)]));
    }

    fn layout(&self, side: Side, vp: Viewport) -> LineLayout<'_> {
        self.folding.layout(side, vp.line_height)
    }

    fn text(&self, side: Side, line: usize) -> &str {
        self.diff.line(side, line)
    }

    /// A valid line outside collapsed folds.
    fn clamp_line(&self, side: Side, line: usize) -> usize {
        let line = line.min(self.diff.line_count(side) - 1);
        self.folding.caret_after(side, line)
    }

    fn cursor_visual(&self, vp: Viewport) -> usize {
        self.layout(self.cursor.side, vp)
            .logical_to_visual(self.cursor.line)
    }

    fn set_line(&mut self, line: usize) {
        self.cursor.line = self.clamp_line(self.cursor.side, line);
    }

    fn set_visual(&mut self, visual: usize, vp: Viewport) {
        let layout = self.layout(self.cursor.side, vp);
        let visual = visual.min(layout.visual_line_count() - 1);
        let line = layout.visual_to_logical(visual);
        self.set_line(line);
    }

    fn cursor_range(&self, scroll_y: f32, viewport: Viewport) -> (usize, usize) {
        scrolloff::cursor_range(&self.layout(self.cursor.side, viewport), scroll_y, viewport)
    }

    fn follow_y(&self, scroll_y: f32, viewport: Viewport, allow_center: bool) -> f32 {
        let layout = self.layout(self.cursor.side, viewport);
        let cursor = self.cursor_visual(viewport);
        scrolloff::follow_y(&layout, cursor, scroll_y, viewport, allow_center)
    }

    /// The cursor owns the master side. The slave follows through sync.
    fn scroll_cursor_side_to(&mut self, y: f32, vp: Viewport) {
        let side = self.cursor.side;
        self.master = side;
        self.scroll
            .scroll_to(&self.diff, &self.folding, side, y, vp);
    }

    fn follow(&mut self, vp: Viewport, allow_center: bool) {
        let y = self.follow_y(self.scroll.y(self.cursor.side), vp, allow_center);
        self.scroll_cursor_side_to(y, vp);
    }

    fn scroll_x(&mut self, x: f32, hx: Horizontal) -> Outcome {
        let before = self.scroll.x;
        self.scroll.scroll_x_to(x, hx.max_x);
        if self.scroll.x == before {
            return Outcome::default();
        }
        Outcome::instant()
    }

    /// The end of the line, one character in from the right edge. A collapsed fold has no text.
    fn scroll_to_end(&mut self, hx: Horizontal) -> Outcome {
        let c = self.cursor;
        let cells = if self.folding.fold_at_line(c.side, c.line).is_some() {
            0
        } else {
            columns::display_width(self.text(c.side, c.line))
        };
        let x = (cells + 1) as f32 * hx.char_width - hx.text_width;
        self.scroll_x(x.max(0.), hx)
    }

    /// After scroll without the cursor: move the cursor in if scrolloff requires it.
    fn push_cursor(&mut self, vp: Viewport) -> bool {
        let (min, max) = self.cursor_range(self.scroll.y(self.cursor.side), vp);
        let cv = self.cursor_visual(vp);
        let target = cv.clamp(min, max);
        if target == cv {
            return false;
        }
        self.set_visual(target, vp);
        true
    }

    pub fn execute(&mut self, cmd: Command, vp: Viewport, hx: Horizontal) -> Outcome {
        use Command::*;
        match cmd {
            MoveDown => self.move_visual(1, vp),
            MoveUp => self.move_visual(-1, vp),
            ScrollLeft => self.scroll_x(self.scroll.x - SCROLL_COLUMNS * hx.char_width, hx),
            ScrollRight => self.scroll_x(self.scroll.x + SCROLL_COLUMNS * hx.char_width, hx),
            ScrollToStart => self.scroll_x(0., hx),
            ScrollToEnd => self.scroll_to_end(hx),
            GoToTop => self.go_to_line(0, vp),
            GoToBottom => {
                let last = self.diff.line_count(self.cursor.side) - 1;
                self.go_to_line(last, vp)
            }
            HalfPageDown => self.half_page(1, vp),
            HalfPageUp => self.half_page(-1, vp),
            NextChange => self.change(true, vp),
            PrevChange => self.change(false, vp),
            FocusSide(side) => self.focus_side(side, vp),
            FoldOpen => self.change_folding(vp, |s, f| f.expand_at(s.0, s.1)),
            ToggleFolds => self.change_folding(vp, |_, f| {
                let any_closed = f.visible(Side::Left).next().is_some();
                f.expand_all(any_closed)
            }),
            // Owned by the UI. After zoom, `rescale` is called.
            CopyLocation | Rollback | ZoomIn | ZoomOut | ResetZoom => Outcome::default(),
        }
    }

    fn move_visual(&mut self, delta: isize, vp: Viewport) -> Outcome {
        let cv = self.cursor_visual(vp);
        let target = cv.saturating_add_signed(delta);
        let before = self.cursor;
        self.set_visual(target, vp);
        if self.cursor == before {
            return Outcome::default();
        }
        self.follow(vp, true);
        Outcome::instant()
    }

    fn go_to_line(&mut self, line: usize, vp: Viewport) -> Outcome {
        self.set_line(line);
        self.follow(vp, true);
        Outcome::animated()
    }

    /// `d` and `u`: scroll and cursor move the same number of lines, vim's `scroll`.
    fn half_page(&mut self, dir: isize, vp: Viewport) -> Outcome {
        let n = (scrolloff::screen_lines(vp) / 2).max(1);
        let side = self.cursor.side;
        let sy = self.scroll.y(side);
        let cv = self.cursor_visual(vp);
        self.set_visual(cv.saturating_add_signed(dir * n as isize), vp);
        let y = self.follow_y(sy + dir as f32 * n as f32 * vp.line_height, vp, false);
        self.scroll_cursor_side_to(y, vp);
        Outcome::animated()
    }

    /// `n` and `p` use IntelliJ's navigation and landing, so the cursor goes to the start of
    /// the hunk. Scroll is only adjusted if the landing breaks scrolloff.
    fn change(&mut self, next: bool, vp: Viewport) -> Outcome {
        let side = self.cursor.side;
        let block = if next {
            next_block(&self.diff, side, self.cursor.line)
        } else {
            prev_block(&self.diff, side, self.cursor.line)
        };
        let Some(block) = block else {
            return Outcome::default();
        };
        let t = target_for_block(&self.diff, &self.folding, block, vp);
        let line = self.clamp_line(side, t.caret_lines[idx(side)]);
        self.cursor = Cursor::new(side, line);
        self.master = side;
        self.scroll.y = t.scroll_y;
        self.scroll.x = t.scroll_x;
        let y = self.follow_y(self.scroll.y(side), vp, false);
        if y != self.scroll.y(side) {
            self.scroll_cursor_side_to(y, vp);
        }
        Outcome::animated()
    }

    /// The line on the other side, via `BaseSyncScrollable.transfer`.
    fn transferred_line(&self) -> usize {
        let c = self.cursor;
        let other = c.side.other();
        let line = self.diff.mapping().transfer(c.side, c.line);
        self.clamp_line(other, line)
    }

    /// `TwosideTextDiffViewer.transferPosition`, without a column.
    pub fn transferred_cursor(&self) -> Cursor {
        Cursor::new(self.cursor.side.other(), self.transferred_line())
    }

    /// The hunk the cursor is in. `None` on a collapsed fold.
    pub fn cursor_block(&self) -> Option<usize> {
        let c = self.cursor;
        if self.folding.fold_at_line(c.side, c.line).is_some() {
            return None;
        }
        self.diff.block_at(c.side, c.line)
    }

    /// The same file with new content. Cursor and scroll are kept as far as possible.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, folding: Rc<Folding>, vp: Viewport) {
        self.folding = folding;
        self.diff = diff;
        let side = self.cursor.side;
        self.cursor = Cursor::new(side, self.clamp_line(side, self.cursor.line));
        self.master = side;
        let y = self.scroll.y(side);
        self.scroll.y[idx(side)] = self.layout(side, vp).clamp_scroll(y, vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, side, vp);
        self.follow(vp, false);
    }

    /// The cursor is set, and the line ends up `offset` pixels below the top of the viewport. For
    /// switching from Unified, so the text under the cursor stays still.
    pub fn place_cursor(&mut self, cursor: Cursor, offset: f32, vp: Viewport) {
        let side = cursor.side;
        self.cursor = Cursor::new(side, self.clamp_line(side, cursor.line));
        self.master = side;
        let layout = self.layout(side, vp);
        let y = layout.line_top(self.cursor.line) - offset;
        self.scroll.y[idx(side)] = layout.clamp_scroll(y, vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, side, vp);
        self.follow(vp, false);
    }

    /// The cursor's line in the newest version.
    pub fn location(&self) -> Cursor {
        if self.cursor.side == Side::Right {
            self.cursor
        } else {
            self.transferred_cursor()
        }
    }

    /// `MyFocusOppositePaneAction`: the cursor moves, the side becomes the master, and then
    /// `scrollToCaret(MAKE_VISIBLE)`.
    fn focus_side(&mut self, side: Side, vp: Viewport) -> Outcome {
        if self.cursor.side == side {
            return Outcome::default();
        }
        self.cursor = self.transferred_cursor();
        self.follow(vp, false);
        Outcome::animated()
    }

    /// `runBatchFoldingOperation` with `PositionKeeper`. The other side's caret is
    /// the transferred position, since we only have one cursor.
    fn change_folding(
        &mut self,
        vp: Viewport,
        op: impl FnOnce((Side, usize), &mut Folding) -> bool,
    ) -> Outcome {
        let side = self.cursor.side;
        let mut carets = [0; 2];
        carets[idx(side)] = self.cursor.line;
        carets[idx(side.other())] = self.transferred_line();
        let keeper = PositionKeeper::save(&self.scroll, &self.folding, carets, vp);
        if !op((side, self.cursor.line), Rc::make_mut(&mut self.folding)) {
            return Outcome::default();
        }
        let after = [Side::Left, Side::Right].map(|s| self.folding.caret_after(s, carets[idx(s)]));
        self.set_line(after[idx(side)]);
        keeper.restore(&mut self.scroll, &self.diff, &self.folding, after, vp);
        Outcome {
            folding_changed: true,
            ..Outcome::instant()
        }
    }

    /// Mouse wheel or trackpad on one side. The cursor is only moved when scrolloff requires it.
    pub fn wheel(&mut self, side: Side, dy: f32, vp: Viewport) -> bool {
        self.master = side;
        self.scroll
            .scroll_by(&self.diff, &self.folding, side, dy, vp);
        self.push_cursor(vp)
    }

    /// A click sets the side and line without scrolling.
    pub fn click(&mut self, side: Side, line: usize) {
        self.master = side;
        self.cursor = Cursor::new(side, self.clamp_line(side, line));
    }

    /// New viewport height: clamp the master, recompute the slave and keep the cursor visible.
    pub fn resize(&mut self, vp: Viewport) {
        let master = self.master;
        let layout = self.layout(master, vp);
        self.scroll.y[idx(master)] = layout.clamp_scroll(self.scroll.y(master), vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, master, vp);
        self.push_cursor(vp);
    }

    /// Zoom: the cursor line is kept at the same place in the viewport, measured in lines.
    pub fn rescale(&mut self, old_line_height: f32, vp: Viewport, x_scale: f32) {
        let side = self.cursor.side;
        let cv = self.cursor_visual(vp) as f32;
        let offset = cv - self.scroll.y(side) / old_line_height;
        let y = (cv - offset) * vp.line_height;
        let y = self.layout(side, vp).clamp_scroll(y, vp.height);
        self.master = side;
        self.scroll.y[idx(side)] = y;
        self.scroll.sync_from(&self.diff, &self.folding, side, vp);
        self.scroll.x *= x_scale;
        self.push_cursor(vp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Command::*;
    use jetdiff_core::ComparisonPolicy;

    const L: Side = Side::Left;
    const R: Side = Side::Right;
    const LH: f32 = 10.;
    /// 20 whole lines.
    const VP: Viewport = Viewport {
        height: 200.,
        line_height: LH,
    };
    const NO_X: Horizontal = Horizontal {
        text_width: 0.,
        char_width: 0.,
        max_x: 0.,
    };

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    fn state(a: &str, b: &str, folds: bool) -> DiffState {
        let diff = FileDiff::new(a, b, ComparisonPolicy::Default).unwrap();
        let folding = if folds {
            Folding::new(&diff)
        } else {
            Folding::none(&diff)
        };
        DiffState::new(Rc::new(diff), Rc::new(folding))
    }

    /// Equal files of 200 lines, no folds.
    fn plain() -> DiffState {
        let a = lines("a", 200);
        state(&a, &a, false)
    }

    /// Hunks on lines 10, 40 and 90 of 100, one line each.
    fn three_changes(folds: bool) -> DiffState {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        state(&a, &(b.join("\n") + "\n"), folds)
    }

    fn run(s: &mut DiffState, cmd: Command) -> Outcome {
        s.execute(cmd, VP, NO_X)
    }

    fn times(s: &mut DiffState, cmd: Command, n: usize) {
        for _ in 0..n {
            run(s, cmd);
        }
    }

    fn top(s: &DiffState) -> f32 {
        s.scroll.y(s.cursor.side)
    }

    /// The cursor has scrolloff, or is at the edge of the file.
    fn assert_scrolloff(s: &DiffState) {
        let (min, max) = s.cursor_range(top(s), VP);
        let cv = s.cursor_visual(VP);
        assert!(min <= cv && cv <= max, "cursor {cv} outside {min}..={max}");
    }

    #[test]
    fn j_and_k() {
        let mut s = plain();
        times(&mut s, MoveDown, 3);
        assert_eq!(s.cursor.line, 3);
        times(&mut s, MoveUp, 10);
        assert_eq!(s.cursor.line, 0);
        assert!(!run(&mut s, MoveUp).changed);
        run(&mut s, GoToBottom);
        assert_eq!(s.cursor.line, 200);
        assert!(!run(&mut s, MoveDown).changed);
    }

    #[test]
    fn scrolloff_on_j() {
        let mut s = plain();
        times(&mut s, MoveDown, 11);
        assert_eq!(top(&s), 0.);
        // Line 12 is eight from the bottom of 20
        run(&mut s, MoveDown);
        assert_eq!(top(&s), 10.);
        assert_scrolloff(&s);
        times(&mut s, MoveUp, 10);
        assert_eq!(s.cursor.line, 2);
        assert_eq!(top(&s), 0.);
    }

    #[test]
    fn j_in_small_window_keeps_scrolloff() {
        let small = Viewport {
            height: 50.,
            line_height: LH,
        };
        let mut s = plain();
        for _ in 0..30 {
            s.execute(MoveDown, small, NO_X);
            let (min, max) = s.cursor_range(top(&s), small);
            let cv = s.cursor_visual(small);
            assert!(min <= cv && cv <= max);
        }
    }

    #[test]
    fn gg_and_g() {
        let mut s = plain();
        let out = run(&mut s, GoToBottom);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 200);
        assert_eq!(top(&s), s.layout(R, VP).max_scroll(VP.height));
        run(&mut s, GoToTop);
        assert_eq!((s.cursor.line, top(&s)), (0, 0.));
    }

    #[test]
    fn long_jump_centers() {
        let mut s = plain();
        s.click(R, 100);
        s.follow(VP, true);
        // 9 lines above the cursor in a window of 20, like vim's `scroll_cursor_halfway`
        assert_eq!(top(&s), 910.);
        assert_scrolloff(&s);
    }

    #[test]
    fn half_page_moves_scroll_and_cursor_equally() {
        let mut s = plain();
        times(&mut s, MoveDown, 10);
        let out = run(&mut s, HalfPageDown);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 20);
        assert_eq!(top(&s), 100.);
        run(&mut s, HalfPageDown);
        assert_eq!((s.cursor.line, top(&s)), (30, 200.));
        run(&mut s, HalfPageUp);
        run(&mut s, HalfPageUp);
        assert_eq!((s.cursor.line, top(&s)), (10, 0.));
        // At the top scroll can't go further, but the cursor can
        run(&mut s, HalfPageUp);
        assert_eq!((s.cursor.line, top(&s)), (0, 0.));
    }

    #[test]
    fn next_and_previous_hunk() {
        let mut s = three_changes(false);
        s.init(VP);
        assert_eq!((s.cursor.side, s.cursor.line), (R, 10));
        let out = run(&mut s, NextChange);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 40);
        assert_scrolloff(&s);
        run(&mut s, PrevChange);
        assert_eq!(s.cursor.line, 10);
        times(&mut s, NextChange, 2);
        assert_eq!(s.cursor.line, 90);
        assert!(!run(&mut s, NextChange).changed);
        times(&mut s, PrevChange, 2);
        assert_eq!(s.cursor.line, 10);
        assert!(!run(&mut s, PrevChange).changed);
    }

    #[test]
    fn hunk_lands_where_intellij_lands() {
        // A window tall enough that the landing doesn't break scrolloff
        let vp = Viewport {
            height: 400.,
            line_height: LH,
        };
        let mut s = three_changes(false);
        s.init(vp);
        s.execute(NextChange, vp, NO_X);
        let t = target_for_block(&s.diff, &s.folding, 1, vp);
        assert_eq!(s.scroll.y, t.scroll_y);
    }

    #[test]
    fn cursor_location_is_in_newest_version() {
        let mut s = state("x\nny1\nny2\nfelles\n", "x\nfelles\n", false);
        s.click(L, 3);
        assert_eq!(s.location(), Cursor::new(R, 1));
        s.click(R, 0);
        assert_eq!(s.location(), Cursor::new(R, 0));
    }

    #[test]
    fn switching_side_transfers_the_line() {
        // Left has three more lines in the hunk
        let a = "x\nny1\nny2\nny3\nfelles æøå\ny\n";
        let b = "x\nfelles æøå\ny\n";
        let mut s = state(a, b, false);
        s.click(L, 4);
        assert!(!run(&mut s, FocusSide(L)).animate);
        let out = run(&mut s, FocusSide(R));
        assert!(out.animate);
        assert_eq!((s.cursor.side, s.cursor.line), (R, 1));
        assert_eq!(s.master, R);
        // A line in a hunk that is empty on Right goes to the line after it
        s.click(L, 2);
        run(&mut s, FocusSide(R));
        assert_eq!((s.cursor.side, s.cursor.line), (R, 1));
        // `transferLine` at the start of an empty hunk side gives the start of the hunk on the other side
        run(&mut s, FocusSide(L));
        assert_eq!((s.cursor.side, s.cursor.line), (L, 1));
    }

    #[test]
    fn j_skips_collapsed_fold() {
        let mut s = three_changes(true);
        // The first fold is 0..6, so line 6
        assert_eq!(s.cursor.line, 0);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 6);
        run(&mut s, MoveUp);
        assert_eq!(s.cursor.line, 0);
        // From 14 to the fold 15..36 and on to 36
        s.click(R, 14);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 15);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 36);
        s.click(R, 20);
        // A click inside a fold lands on its start
        assert_eq!(s.cursor.line, 15);
    }

    #[test]
    fn fold_commands_keep_the_cursor() {
        let mut s = three_changes(true);
        s.click(R, 45);
        let y = top(&s);
        let out = run(&mut s, FoldOpen);
        assert!(out.folding_changed);
        assert_eq!(s.cursor.line, 45);
        assert_eq!(top(&s), y);
        assert!(s.folding.fold_at_line(R, 49).is_some());
        run(&mut s, ToggleFolds);
        assert_eq!(s.folding.visible(R).count(), 0);
        times(&mut s, MoveDown, 20);
        run(&mut s, ToggleFolds);
        assert!(s.folding.visible(R).count() > 0);
        // The cursor was inside a fold that closed
        assert_eq!(s.cursor.line, 45);
    }

    #[test]
    fn mouse_wheel_pushes_the_cursor() {
        let mut s = plain();
        times(&mut s, MoveDown, 11);
        assert!(!s.wheel(L, 30., VP));
        assert_eq!(s.cursor.line, 11);
        assert!(s.wheel(L, 100., VP));
        assert_eq!(s.cursor.line, 21);
        assert_eq!(s.master, L);
        // A fraction of a line from the trackpad
        s.wheel(R, 3.5, VP);
        assert_scrolloff(&s);
    }

    #[test]
    fn sideways_scroll() {
        let long = format!("{}\n", "x".repeat(200));
        let mut s = state(&long, &long, false);
        let hx = Horizontal {
            text_width: 100.,
            char_width: 5.,
            max_x: 905.,
        };
        s.execute(ScrollRight, VP, hx);
        assert_eq!(s.scroll.x, 20.);
        s.execute(ScrollToEnd, VP, hx);
        assert_eq!(s.scroll.x, 905.);
        assert!(!s.execute(ScrollRight, VP, hx).changed);
        s.execute(ScrollLeft, VP, hx);
        assert_eq!(s.scroll.x, 885.);
        s.execute(ScrollToStart, VP, hx);
        assert_eq!(s.scroll.x, 0.);
        assert!(!s.execute(ScrollLeft, VP, hx).changed);
        // The line at the bottom is empty, so `$` goes all the way to the left
        run(&mut s, GoToBottom);
        s.execute(ScrollRight, VP, hx);
        s.execute(ScrollToEnd, VP, hx);
        assert_eq!(s.scroll.x, 0.);
    }

    #[test]
    fn zoom_keeps_cursor_line_in_place() {
        let mut s = plain();
        times(&mut s, MoveDown, 50);
        let before = s.cursor_visual(VP) as f32 - top(&s) / LH;
        let big = Viewport {
            height: 400.,
            line_height: 20.,
        };
        s.rescale(LH, big, 2.);
        let after = s.cursor_visual(big) as f32 - top(&s) / 20.;
        assert_eq!(before, after);
    }

    #[test]
    fn resize_keeps_cursor_visible() {
        let mut s = plain();
        times(&mut s, MoveDown, 18);
        let small = Viewport {
            height: 100.,
            line_height: LH,
        };
        s.resize(small);
        let (min, max) = s.cursor_range(top(&s), small);
        let cv = s.cursor_visual(small);
        assert!(min <= cv && cv <= max);
    }

    #[test]
    fn rollback_keeps_cursor_and_scroll() {
        // Close enough that the gap 37..44 is not folded after 40 is rolled back
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [36, 40, 44] {
            b[i] = format!("x{i}");
        }
        let mut s = state(&a, &(b.join("\n") + "\n"), true);
        run(&mut s, NextChange);
        run(&mut s, NextChange);
        assert_eq!(s.cursor, Cursor::new(R, 40));
        assert_eq!(s.cursor_block(), Some(1));
        let y = top(&s);
        let a = s.diff.text(L).to_owned();
        let text = s.diff.text(R).replace("x40\n", "a40\n");
        let diff = FileDiff::new(&a, &text, ComparisonPolicy::Default).unwrap();
        let folding = Rc::new(Folding::new(&diff));
        s.replace_diff(Rc::new(diff), folding, VP);
        assert_eq!(s.diff.blocks().len(), 2);
        assert_eq!(s.cursor, Cursor::new(R, 40));
        assert_eq!(top(&s), y);
        assert_eq!(s.cursor_block(), None);
    }
}
