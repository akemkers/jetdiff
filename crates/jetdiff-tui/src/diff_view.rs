//! Diffen av én fil, som `FileDiffView` i GPUI. Split er justerte rader og har ingen folding.

use std::rc::Rc;

use jetdiff_app::{RollbackRequest, location, rollback_request};
use jetdiff_viewmodel::{
    Command, FileDiff, Horizontal, Key, KeyCode, KeyParser, Parsed, Theme, UnifiedDiff,
    UnifiedState, Viewport,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::WHEEL_ROWS;
use crate::geometry;
use crate::paint::{paint, row_at};

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
    state: UnifiedState,
    keys: KeyParser,
    unified: bool,
    can_rollback: bool,
    pub overlay: Option<Overlay>,
    paths: [String; 2],
    max_cols: usize,
    initialized: bool,
    area: Rect,
}

impl DiffView {
    pub fn new(diff: FileDiff, paths: [String; 2], unified: bool) -> Self {
        Self {
            max_cols: diff.max_display_width(),
            state: state(Rc::new(diff), unified),
            keys: KeyParser::default(),
            unified,
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
        self.state = state(Rc::new(diff), self.unified);
        self.keys.reset();
        self.initialized = false;
    }

    /// Cursor og scroll beholdes.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2]) {
        let Some(viewport) = self.viewport().filter(|_| self.initialized) else {
            return self.set_diff(diff, paths);
        };
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        let aligned = aligned(&diff, self.unified);
        if aligned != self.state.diff.is_aligned() {
            self.state = self.state.realign(aligned, viewport);
        }
        self.state.replace_diff(Rc::new(diff), viewport);
        self.clamp_scroll_x();
    }

    fn viewport(&self) -> Option<Viewport> {
        (self.area.height > 0).then_some(Viewport {
            height: self.area.height as f32,
            line_height: 1.,
        })
    }

    fn text_width(&self) -> u16 {
        let diff = &self.state.diff.diff;
        if self.state.diff.is_aligned() {
            geometry::aligned(diff, self.area).sides[0].text_width()
        } else {
            geometry::unified(diff, self.area).columns.text_width()
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

    /// Før tegning.
    pub fn layout(&mut self, area: Rect) {
        let resized = area.height != self.area.height;
        self.area = area;
        let Some(viewport) = self.viewport() else {
            return;
        };
        if !self.initialized {
            self.initialized = true;
            self.state.init(viewport);
        } else if resized {
            self.state.resize(viewport);
        }
        self.clamp_scroll_x();
    }

    pub fn toggle_view(&mut self) -> Option<Effect> {
        if !self.state.diff.diff.can_split() {
            return None;
        }
        self.unified = !self.unified;
        match self.viewport().filter(|_| self.initialized) {
            Some(viewport) => self.state = self.state.realign(!self.unified, viewport),
            None => self.state = state(self.state.diff.diff.clone(), self.unified),
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
            Command::Rollback => {
                let diff = &self.state.diff.diff;
                match rollback_request(diff, self.state.cursor_block(), self.can_rollback) {
                    Ok(request) => {
                        self.overlay = Some(Overlay::Confirm(request));
                        None
                    }
                    Err(text) => Some(Effect::Message(text.into())),
                }
            }
            Command::CopyLocation => location(self.state.location(), &self.paths).map(Effect::Copy),
            _ => {
                let viewport = self.viewport()?;
                self.state.execute(cmd, viewport, self.horizontal());
                None
            }
        }
    }

    pub fn click(&mut self, x: u16, y: u16) {
        self.keys.reset();
        self.overlay = None;
        if !self.area.contains((x, y).into()) {
            return;
        }
        let layout = self.state.diff.layout(1.);
        let screen = y - self.area.y;
        if let Some(row) = row_at(&layout, self.state.scroll_y(), screen) {
            self.state.click(row);
        }
    }

    pub fn wheel(&mut self, down: bool) {
        if let Some(viewport) = self.viewport() {
            let rows = WHEEL_ROWS as f32;
            self.state.wheel(if down { rows } else { -rows }, viewport);
        }
    }

    pub fn render(&self, focused: bool, theme: &Theme, buf: &mut Buffer) {
        paint(&self.state, focused, self.area, theme, buf);
    }
}

fn aligned(diff: &FileDiff, unified: bool) -> bool {
    !unified && diff.can_split()
}

fn state(diff: Rc<FileDiff>, unified: bool) -> UnifiedState {
    let rows = if aligned(&diff, unified) {
        UnifiedDiff::aligned(diff)
    } else {
        UnifiedDiff::new(diff)
    };
    UnifiedState::new(Rc::new(rows))
}
