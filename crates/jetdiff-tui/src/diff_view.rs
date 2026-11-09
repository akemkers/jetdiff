//! The diff of one file, like `FileDiffView` in GPUI. Split is aligned rows, and both diff modes
//! fold the unchanged rows.

mod split_share;

use std::rc::Rc;
use std::time::Instant;

use jetdiff_app::{RollbackRequest, cursor_location, rollback_request};
use jetdiff_viewmodel::{
    Command, FileDiff, Horizontal, Key, KeyCode, KeyParser, Parsed, RowLayout, ScrollAnimator,
    SplitShare, Theme, UnifiedDiff, UnifiedState, Viewport,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::WHEEL_ROWS;
use crate::diff_divider_grab::DiffDividerGrab;
use crate::{code_viewer, geometry};

/// Longer than in GPUI, since whole rows need more time to look smooth.
const NAV_ANIMATION_MS: f32 = 250.;

pub enum Overlay {
    /// Enter confirms, anything else cancels.
    Confirm(RollbackRequest),
    Help,
}

pub enum Effect {
    /// Shown as a brief info notification.
    ShowBrief(String),
    Copy(String),
    Rollback(RollbackRequest),
}

pub struct DiffView {
    state: UnifiedState,
    /// What is shown. The target is `state.scroll`.
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
    pub overlay: Option<Overlay>,
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
            state: state(Rc::new(diff), unified, fold_unchanged),
            scroll: ScrollAnimator::in_whole_cells(NAV_ANIMATION_MS),
            smooth_scroll,
            keys: KeyParser::default(),
            unified,
            fold_unchanged,
            split_share: SplitShare::default(),
            diff_divider_grab: DiffDividerGrab::default(),
            can_rollback: false,
            overlay: None,
            paths,
            initialized: false,
            area: Rect::default(),
        }
    }

    pub fn set_can_rollback(&mut self, can_rollback: bool) {
        self.can_rollback = can_rollback;
    }

    pub fn set_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        if matches!(self.overlay, Some(Overlay::Confirm(_))) {
            self.overlay = None;
        }
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        self.state = state(Rc::new(diff), self.unified, self.fold_unchanged);
        self.scroll.jump(self.state.scroll);
        self.keys.reset();
        self.initialized = false;
    }

    /// Cursor and scroll are kept.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        let Some(viewport) = self.viewport().filter(|_| self.initialized) else {
            return self.set_diff(diff, paths);
        };
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        let row_layout = row_layout(&diff, self.unified);
        if row_layout != self.state.diff.row_layout() {
            self.state = self
                .state
                .realign(row_layout, self.fold_unchanged, viewport);
        }
        self.state
            .replace_diff(Rc::new(diff), self.fold_unchanged, viewport);
        self.clamp_scroll_x();
        self.scroll.retarget(self.state.scroll, Instant::now());
    }

    fn viewport(&self) -> Option<Viewport> {
        (self.area.height > 0).then_some(Viewport {
            height: self.area.height as f32,
            line_height: 1.,
        })
    }

    fn text_width(&self) -> u16 {
        let diff = &self.state.diff.diff;
        match self.state.diff.row_layout() {
            RowLayout::Split => geometry::aligned(diff, self.area, self.split_share).text_width(),
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
        self.state.scroll.scroll_x_to(self.state.scroll.x, max_x);
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
            self.state.init(viewport);
        } else if resized {
            self.state.resize(viewport);
        } else {
            return;
        }
        self.clamp_scroll_x();
        self.scroll.jump(self.state.scroll);
    }

    pub fn toggle_diff_mode(&mut self) -> Option<Effect> {
        if !self.state.diff.diff.can_split() {
            return None;
        }
        self.unified = !self.unified;
        self.scroll.stop(&mut self.state.scroll);
        match self.viewport().filter(|_| self.initialized) {
            Some(viewport) => {
                let row_layout = row_layout(&self.state.diff.diff, self.unified);
                self.state = self
                    .state
                    .realign(row_layout, self.fold_unchanged, viewport);
            }
            None => {
                let diff = self.state.diff.diff.clone();
                self.state = state(diff, self.unified, self.fold_unchanged);
            }
        }
        self.clamp_scroll_x();
        self.scroll.jump(self.state.scroll);
        Some(Effect::ShowBrief(
            if self.unified { "Unified" } else { "Split" }.into(),
        ))
    }

    pub fn toggle_help(&mut self) {
        self.overlay = match self.overlay {
            Some(Overlay::Help) => None,
            _ => Some(Overlay::Help),
        };
    }

    /// `false` when help is not shown, so Esc passes through.
    pub fn hide_help(&mut self) -> bool {
        if !matches!(self.overlay, Some(Overlay::Help)) {
            return false;
        }
        self.overlay = None;
        true
    }

    pub fn key(&mut self, key: Key) -> Option<Effect> {
        if let Some(Overlay::Confirm(request)) = self.overlay.take() {
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
                let diff = &self.state.diff.diff;
                match rollback_request(diff, self.state.cursor_hunk(), self.can_rollback) {
                    Ok(request) => {
                        self.overlay = Some(Overlay::Confirm(request));
                        None
                    }
                    Err(text) => Some(Effect::ShowBrief(text.into())),
                }
            }
            Command::CopyCursorLocation => {
                cursor_location(self.state.cursor_location(), &self.paths).map(Effect::Copy)
            }
            _ => {
                let viewport = self.viewport()?;
                let outcome = self.state.execute(cmd, viewport, self.horizontal());
                if outcome.animate && self.smooth_scroll {
                    self.scroll.animate(self.state.scroll, Instant::now());
                } else if outcome.changed {
                    self.scroll.retarget(self.state.scroll, Instant::now());
                }
                if cmd != Command::ToggleFolds || !outcome.changed {
                    return None;
                }
                self.fold_unchanged = self.state.folding.any_closed();
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
        self.overlay = None;
        if self.grab_diff_divider(x, y, now) || !self.area.contains((x, y).into()) {
            return;
        }
        self.scroll.stop(&mut self.state.scroll);
        let layout = self.state.folding.layout(1.);
        let screen = y - self.area.y;
        let Some(row) = code_viewer::row_at(&layout, self.state.scroll_y(), screen) else {
            return;
        };
        self.state.click(row);
        if self.state.folding.fold_at_row(row).is_some() {
            self.execute(Command::FoldOpen);
        }
    }

    pub fn wheel(&mut self, down: bool) {
        if let Some(viewport) = self.viewport() {
            self.scroll.stop(&mut self.state.scroll);
            let rows = WHEEL_ROWS as f32;
            self.state.wheel(if down { rows } else { -rows }, viewport);
            self.scroll.jump(self.state.scroll);
        }
    }

    pub fn render(&self, focused: bool, theme: &Theme, buf: &mut Buffer) {
        let shown = UnifiedState {
            scroll: self.scroll.shown(),
            ..self.state.clone()
        };
        code_viewer::paint(&shown, self.split_share, focused, self.area, theme, buf);
    }
}

/// Split is aligned rows.
fn row_layout(diff: &FileDiff, unified: bool) -> RowLayout {
    if !unified && diff.can_split() {
        RowLayout::Split
    } else {
        RowLayout::Unified
    }
}

fn state(diff: Rc<FileDiff>, unified: bool, fold_unchanged: bool) -> UnifiedState {
    let row_layout = row_layout(&diff, unified);
    UnifiedState::new(
        Rc::new(UnifiedDiff::with_layout(diff, row_layout)),
        fold_unchanged,
    )
}
