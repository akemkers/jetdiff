//! The diff of one file, like `FileDiffView` in GPUI. Split is Aligned Split, and both diff modes
//! fold the unchanged rows.

mod split_share;

use std::rc::Rc;
use std::time::Instant;

use jetdiff_app::{Overlay, OverlaySlot, RollbackRequest, cursor_location, rollback_request};
use jetdiff_viewmodel::{
    Command, DiffMode, FileDiff, Horizontal, Key, KeyCode, KeyParser, ModeState, Parsed,
    PointerTarget, RowLayout, ScrollAnimator, SplitRows, SplitShare, Theme, UnifiedState, Viewport,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::WHEEL_ROWS;
use crate::diff_divider_grab::DiffDividerGrab;
use crate::{code_viewer, geometry};

/// Longer than in GPUI, since whole rows need more time to look smooth.
const NAV_ANIMATION_MS: f32 = 250.;

pub enum Effect {
    /// Shown as a brief info notification.
    ShowBrief(String),
    Copy(String),
    Rollback(RollbackRequest),
}

pub struct DiffView {
    mode: DiffMode,
    /// What is shown. The target is `mode.scroll()`.
    scroll: ScrollAnimator,
    smooth_scroll: bool,
    keys: KeyParser,
    unified: bool,
    /// Folds closed for all files, until `f` switches.
    fold_unchanged: bool,
    /// Left's share of the width in Split, for all files until jetdiff is closed.
    split_share: SplitShare,
    diff_divider_grab: DiffDividerGrab,
    can_rollback: bool,
    overlay: OverlaySlot,
    paths: [String; 2],
    max_cols: usize,
    initialized: bool,
    area: Rect,
}

impl DiffView {
    pub fn new(
        diff: FileDiff,
        paths: [String; 2],
        unified: bool,
        fold_unchanged: bool,
        smooth_scroll: bool,
    ) -> Self {
        Self {
            max_cols: diff.max_display_width(),
            mode: DiffMode::new(Rc::new(diff), unified, fold_unchanged, SplitRows::Aligned),
            scroll: ScrollAnimator::in_whole_cells(NAV_ANIMATION_MS),
            smooth_scroll,
            keys: KeyParser::default(),
            unified,
            fold_unchanged,
            split_share: SplitShare::default(),
            diff_divider_grab: DiffDividerGrab::default(),
            can_rollback: false,
            overlay: OverlaySlot::default(),
            paths,
            initialized: false,
            area: Rect::default(),
        }
    }

    pub fn set_can_rollback(&mut self, can_rollback: bool) {
        self.can_rollback = can_rollback;
    }

    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.current()
    }

    pub fn set_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        self.overlay.cancel_rollback();
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        self.mode = DiffMode::new(
            Rc::new(diff),
            self.unified,
            self.fold_unchanged,
            SplitRows::Aligned,
        );
        self.scroll.jump(self.mode.scroll());
        self.keys.reset();
        self.initialized = false;
    }

    /// Cursor and scroll are kept.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        let Some(viewport) = self.viewport().filter(|_| self.initialized) else {
            return self.set_diff(diff, paths);
        };
        self.overlay.cancel_rollback();
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        self.mode
            .replace_diff(Rc::new(diff), self.unified, self.fold_unchanged, viewport);
        self.clamp_scroll_x();
        self.scroll.retarget(self.mode.scroll(), Instant::now());
    }

    fn viewport(&self) -> Option<Viewport> {
        (self.area.height > 0).then_some(Viewport {
            height: self.area.height as f32,
            line_height: 1.,
        })
    }

    fn text_width(&self) -> u16 {
        let rows = self.rows();
        let diff = &rows.diff.diff;
        match rows.diff.row_layout() {
            RowLayout::AlignedSplit => {
                geometry::aligned(diff, self.area, self.split_share).text_width()
            }
            RowLayout::Unified => geometry::unified(diff, self.area).columns.text_width(),
        }
    }

    fn horizontal(&self) -> Horizontal {
        Horizontal {
            text_width: self.text_width() as f32,
            char_width: 1.,
            max_x: self.max_scroll_x(),
        }
    }

    fn max_scroll_x(&self) -> f32 {
        (self.max_cols as f32 + 1. - self.text_width() as f32).max(0.)
    }

    fn clamp_scroll_x(&mut self) {
        let max_x = self.max_scroll_x();
        let scroll = self.mode.scroll_mut();
        scroll.scroll_x_to(scroll.x, max_x);
    }

    /// Before drawing.
    pub fn layout(&mut self, area: Rect) {
        let resized = area.as_size() != self.area.as_size();
        self.area = area;
        let Some(viewport) = self.viewport() else {
            return;
        };
        if !self.initialized {
            self.initialized = true;
            self.mode.init(viewport);
        } else if resized {
            self.mode.resize(viewport);
        } else {
            return;
        }
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
    }

    pub fn toggle_diff_mode(&mut self) -> Option<Effect> {
        if !self.mode.diff().can_split() {
            return None;
        }
        self.unified = !self.unified;
        self.scroll.stop(self.mode.scroll_mut());
        match self.viewport().filter(|_| self.initialized) {
            Some(viewport) => self.mode.toggle(self.fold_unchanged, viewport),
            None => {
                let diff = self.mode.diff().clone();
                self.mode =
                    DiffMode::new(diff, self.unified, self.fold_unchanged, SplitRows::Aligned);
            }
        }
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
        Some(Effect::ShowBrief(
            if self.unified { "Unified" } else { "Split" }.into(),
        ))
    }

    pub fn toggle_help(&mut self) {
        self.overlay.toggle_help();
    }

    /// `false` when help is not shown, so Esc passes through.
    pub fn hide_help(&mut self) -> bool {
        self.overlay.hide_help()
    }

    pub fn key(&mut self, key: Key) -> Option<Effect> {
        if let Some(request) = self.overlay.take_rollback() {
            let plain = !key.ctrl && !key.shift && !key.cmd;
            return (key.code == KeyCode::Enter && plain).then_some(Effect::Rollback(request));
        }
        match self.keys.feed(key) {
            Parsed::Command(cmd) => self.execute(cmd),
            Parsed::Pending | Parsed::Ignored => None,
        }
    }

    fn execute(&mut self, cmd: Command) -> Option<Effect> {
        match cmd {
            Command::Rollback => {
                let diff = self.mode.diff();
                match rollback_request(diff, self.mode.cursor_hunk(), self.can_rollback) {
                    Ok(request) => {
                        self.overlay.ask_rollback(request);
                        None
                    }
                    Err(text) => Some(Effect::ShowBrief(text.into())),
                }
            }
            Command::CopyCursorLocation => {
                cursor_location(self.mode.cursor_location(), &self.paths).map(Effect::Copy)
            }
            _ => {
                let viewport = self.viewport()?;
                let outcome = self.mode.execute(cmd, viewport, self.horizontal());
                if outcome.animate && self.smooth_scroll {
                    self.scroll.animate(self.mode.scroll(), Instant::now());
                } else if outcome.changed {
                    self.scroll.retarget(self.mode.scroll(), Instant::now());
                }
                if cmd != Command::ToggleFolds || !outcome.changed {
                    return None;
                }
                self.fold_unchanged = self.mode.any_fold_closed();
                let text = if self.fold_unchanged {
                    "Folds closed"
                } else {
                    "Folds opened"
                };
                Some(Effect::ShowBrief(text.into()))
            }
        }
    }

    pub fn is_animating(&self) -> bool {
        self.scroll.is_animating()
    }

    /// `true` when the row shown has changed and a new frame is needed.
    pub fn step_animation(&mut self, now: Instant) -> bool {
        self.scroll.step(now)
    }

    /// A click on a closed fold opens it, as in GPUI.
    pub fn click(&mut self, x: u16, y: u16, now: Instant) {
        self.keys.reset();
        self.overlay.close();
        if self.grab_diff_divider(x, y, now) || !self.area.contains((x, y).into()) {
            return;
        }
        self.scroll.stop(self.mode.scroll_mut());
        let rows = self.rows();
        let layout = rows.folding.layout(1.);
        let screen = y - self.area.y;
        let Some(row) = code_viewer::row_at(&layout, rows.scroll_y(), screen) else {
            return;
        };
        let target = PointerTarget::Row(row);
        self.mode.click(target);
        if self.mode.fold_at(target).is_some() {
            self.execute(Command::FoldOpen);
        }
    }

    pub fn wheel(&mut self, down: bool) {
        if let Some(viewport) = self.viewport() {
            self.scroll.stop(self.mode.scroll_mut());
            let rows = WHEEL_ROWS as f32;
            self.mode
                .wheel(None, if down { rows } else { -rows }, viewport);
            self.scroll.jump(self.mode.scroll());
        }
    }

    pub fn render(&self, focused: bool, theme: &Theme, buf: &mut Buffer) {
        let shown = UnifiedState {
            scroll: self.scroll.shown(),
            ..self.rows().clone()
        };
        code_viewer::paint(&shown, self.split_share, focused, self.area, theme, buf);
    }

    /// The terminal shows Split as Aligned Split, so both diff modes are rows.
    fn rows(&self) -> &UnifiedState {
        match self.mode.state() {
            ModeState::Rows(state) => state,
            ModeState::Split(_) => unreachable!("DiffMode is made with SplitRows::Aligned"),
        }
    }
}
