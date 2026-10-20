//! Diffen av én fil, som `FileDiffView` i GPUI.

use std::rc::Rc;

use jetdiff_app::{RollbackRequest, location, rollback_request};
use jetdiff_viewmodel::columns::display_width;
use jetdiff_viewmodel::{
    Command, DiffMode, FileDiff, Folding, Horizontal, Key, KeyCode, KeyParser, Parsed, Side, Theme,
    Viewport,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::geometry;
use crate::paint::{RowContent, paint_split, paint_unified, row_content};

const WHEEL_LINES: f32 = 3.;

pub enum Overlay {
    /// Enter bekrefter, alt annet avbryter.
    Confirm(RollbackRequest),
    Help,
}

pub enum Effect {
    Message(String),
    Copy(String),
    Rollback(RollbackRequest),
}

pub struct DiffView {
    mode: DiffMode,
    keys: KeyParser,
    unified: bool,
    collapse_unchanged: bool,
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
        collapse_unchanged: bool,
    ) -> Self {
        Self {
            max_cols: max_cols(&diff),
            mode: diff_mode(diff, unified, collapse_unchanged),
            keys: KeyParser::default(),
            unified,
            collapse_unchanged,
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
        self.max_cols = max_cols(&diff);
        self.mode = diff_mode(diff, self.unified, self.collapse_unchanged);
        self.keys.reset();
        self.initialized = false;
    }

    /// Cursor og scroll beholdes.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        let Some(vp) = self.viewport().filter(|_| self.initialized) else {
            return self.set_diff(diff, paths);
        };
        self.paths = paths;
        self.max_cols = max_cols(&diff);
        let folding = folding(&diff, self.collapse_unchanged);
        self.mode
            .replace_diff(Rc::new(diff), Rc::new(folding), self.unified, vp);
        self.clamp_scroll_x();
    }

    fn viewport(&self) -> Option<Viewport> {
        (self.area.height > 0).then_some(Viewport {
            height: self.area.height as f32,
            line_height: 1.,
        })
    }

    fn text_width(&self) -> u16 {
        match &self.mode {
            DiffMode::Split(state) => geometry::split(&state.diff, self.area)[0].text_width(),
            DiffMode::Unified(state) => geometry::unified(&state.diff.diff, self.area)
                .editor
                .text_width(),
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

    pub fn layout(&mut self, area: Rect) {
        let resized = area.height != self.area.height;
        self.area = area;
        let Some(vp) = self.viewport() else {
            return;
        };
        if !self.initialized {
            self.initialized = true;
            self.mode.init(vp);
        } else if resized {
            self.mode.resize(vp);
        }
        self.clamp_scroll_x();
    }

    pub fn toggle_view(&mut self) -> Option<Effect> {
        if self.mode.diff().single_side().is_some() {
            return None;
        }
        self.unified = !self.unified;
        let diff = self.mode.diff().clone();
        let folding = Rc::new(folding(&diff, self.collapse_unchanged));
        match self.viewport().filter(|_| self.initialized) {
            Some(vp) => self.mode.toggle(folding, vp),
            None => self.mode = DiffMode::new(diff, folding, self.unified),
        }
        self.clamp_scroll_x();
        Some(Effect::Message(
            if self.unified { "Unified" } else { "Split" }.into(),
        ))
    }

    pub fn toggle_help(&mut self) {
        self.overlay = match self.overlay {
            Some(Overlay::Help) => None,
            _ => Some(Overlay::Help),
        };
    }

    /// `false` når hjelpen ikke vises, så Esc går videre.
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
            // En terminal har én fontstørrelse
            Command::ZoomIn | Command::ZoomOut | Command::ResetZoom => return None,
            Command::Rollback => {
                return match rollback_request(&self.mode, self.can_rollback) {
                    Ok(request) => {
                        self.overlay = Some(Overlay::Confirm(request));
                        None
                    }
                    Err(text) => Some(Effect::Message(text.into())),
                };
            }
            Command::CopyLocation => return location(&self.mode, &self.paths).map(Effect::Copy),
            _ => {}
        }
        let vp = self.viewport()?;
        let out = self.mode.execute(cmd, vp, self.horizontal());
        if let (Command::ToggleFolds, true, DiffMode::Split(state)) = (cmd, out.changed, &self.mode)
        {
            let closed = state.folding.visible(Side::Left).next().is_some();
            return Some(Effect::Message(
                if closed {
                    "Folds lukket"
                } else {
                    "Folds åpnet"
                }
                .into(),
            ));
        }
        None
    }

    /// Klikk på en fold åpner neste nivå.
    pub fn click(&mut self, x: u16, y: u16) {
        self.keys.reset();
        self.overlay = None;
        if !self.area.contains((x, y).into()) {
            return;
        }
        let row = y - self.area.y;
        match &mut self.mode {
            DiffMode::Split(state) => {
                let sides = geometry::split(&state.diff, self.area);
                let Some(cells) = sides.iter().find(|c| c.contains(x)) else {
                    return;
                };
                let side = cells.side;
                let layout = state.folding.layout(side, 1.);
                let line = layout
                    .y_to_logical_line(state.scroll.y(side).round() + row as f32)
                    .min(layout.line_count() - 1);
                let fold = matches!(
                    row_content(&layout, state.scroll.y(side), row),
                    RowContent::Fold
                );
                state.click(side, line);
                if fold {
                    self.execute(Command::FoldOpen);
                }
            }
            DiffMode::Unified(state) => {
                let layout = state.diff.layout(1.);
                let index = layout.y_to_logical_line(state.scroll_y().round() + row as f32);
                state.click(index.min(layout.line_count() - 1));
            }
        }
    }

    pub fn wheel(&mut self, x: u16, down: bool) {
        let Some(vp) = self.viewport() else {
            return;
        };
        let dy = if down { WHEEL_LINES } else { -WHEEL_LINES };
        match &mut self.mode {
            DiffMode::Split(state) => {
                let middle = self.area.x + self.area.width / 2;
                let side = if x < middle { Side::Left } else { Side::Right };
                state.wheel(side, dy, vp);
            }
            DiffMode::Unified(state) => {
                state.wheel(dy, vp);
            }
        }
    }

    pub fn render(&self, focused: bool, theme: &Theme, buf: &mut Buffer) {
        match &self.mode {
            DiffMode::Split(state) => paint_split(state, focused, self.area, theme, buf),
            DiffMode::Unified(state) => paint_unified(state, focused, self.area, theme, buf),
        }
    }
}

fn diff_mode(diff: FileDiff, unified: bool, collapse_unchanged: bool) -> DiffMode {
    let folding = folding(&diff, collapse_unchanged);
    DiffMode::new(Rc::new(diff), Rc::new(folding), unified)
}

fn folding(diff: &FileDiff, collapse_unchanged: bool) -> Folding {
    let mut folding = Folding::new(diff);
    folding.expand_all(!collapse_unchanged);
    folding
}

fn max_cols(diff: &FileDiff) -> usize {
    [Side::Left, Side::Right]
        .iter()
        .flat_map(|&s| (0..diff.line_count(s)).map(move |l| (s, l)))
        .map(|(s, l)| display_width(diff.line(s, l)))
        .max()
        .unwrap_or(0)
}
