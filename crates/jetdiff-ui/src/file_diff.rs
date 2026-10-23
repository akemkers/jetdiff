//! The diff of one file, Split or Unified. `DiffMode` owns the cursor, folding and the scroll target.
//! This is where the parts that need GPUI live: keys, mouse, animation toward the target and zoom.

use std::rc::Rc;

use gpui::{
    App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ScrollWheelEvent,
    SharedString, Window, actions, canvas, div, prelude::*, px,
};
use jetdiff_app::{RollbackRequest, location, rollback_request};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::metrics::zoomed;
use jetdiff_viewmodel::{
    Command, DiffMode, FileDiff, FoldId, Folding, Horizontal, KeyParser, Parsed, StripeMarker,
    Viewport, stripe_markers,
};

use crate::code_viewer::{self, Hit, Painter, Shared};
use crate::help::{BAR_HEIGHT, HideHelp, KeyHelp, ToggleHelp};
use crate::keystroke::to_key;
use crate::message::{Level, Message};
use crate::rollback_dialog::rollback_dialog;
use crate::scroll_animator::ScrollAnimator;
use crate::settings::Settings;
use crate::text::TextStyle;

actions!(jetdiff, [ZoomIn, ZoomOut, ResetZoom, ToggleView]);

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// The user wants to roll back a hunk. Sent to whoever can write the file.
pub struct Rollback(pub RollbackRequest);

/// Lies over the diff. Only one at a time.
enum Overlay {
    /// Waiting for confirmation of the rollback. Enter confirms, anything else cancels.
    Confirm(Rollback),
    Help,
}

/// New font size after zoom, so the rest of the window can follow.
pub struct Zoomed {
    pub font_size: f32,
}

pub struct FileDiffView {
    mode: DiffMode,
    font_size: f32,
    style: Option<TextStyle>,
    /// What is shown now. The target is `mode.scroll()`.
    scroll: ScrollAnimator,
    keys: KeyParser,
    initialized: bool,
    viewport_height: f32,
    markers: Rc<[Vec<StripeMarker>; 2]>,
    /// The longest line in characters, computed once. Gives max horizontal scroll without shaping everything.
    max_cols: usize,
    /// Split or Unified for all files, until `v` switches.
    unified: bool,
    /// Right is a file in Local changes, so hunks can be rolled back.
    can_rollback: bool,
    overlay: Option<Overlay>,
    /// Left and Right path for `y`. Empty when there is no file.
    paths: [String; 2],
    /// Owned by whoever makes the diff, but drawn above the keymap bar here.
    message: Entity<Message>,
    shared: Rc<Shared>,
    focus: FocusHandle,
}

impl FileDiffView {
    /// One file, with its own messages.
    pub fn standalone(diff: FileDiff, paths: [String; 2], cx: &mut Context<Self>) -> Self {
        let message = cx.new(|_| Message::default());
        Self::new(diff, paths, message, cx)
    }

    pub(crate) fn new(
        diff: FileDiff,
        paths: [String; 2],
        message: Entity<Message>,
        cx: &mut Context<Self>,
    ) -> Self {
        let unified = cx.global::<Settings>().unified;
        Self {
            max_cols: diff.max_display_width(),
            mode: diff_mode(diff, unified, cx),
            unified,
            font_size: cx.global::<Settings>().font_size,
            style: None,
            scroll: ScrollAnimator::default(),
            keys: KeyParser::default(),
            initialized: false,
            viewport_height: 0.,
            markers: Rc::new([Vec::new(), Vec::new()]),
            can_rollback: false,
            overlay: None,
            paths,
            message,
            shared: Rc::new(Shared::new()),
            focus: cx.focus_handle(),
        }
    }

    pub fn set_can_rollback(&mut self, can_rollback: bool) {
        self.can_rollback = can_rollback;
    }

    /// A new file in the same view. Zoom is kept; cursor and scroll start over.
    pub fn set_diff(&mut self, diff: FileDiff, paths: [String; 2], cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Confirm(_))) {
            self.overlay = None;
        }
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        self.mode = diff_mode(diff, self.unified, cx);
        self.scroll = ScrollAnimator::default();
        self.keys.reset();
        self.initialized = false;
        self.shared.clear();
        self.update_markers();
        cx.notify();
    }

    /// The same file with new content, after a rollback or a save. Cursor and scroll are kept.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2], cx: &mut Context<Self>) {
        let Some(vp) = self.viewport().filter(|_| self.initialized) else {
            self.set_diff(diff, paths, cx);
            return;
        };
        self.paths = paths;
        self.max_cols = diff.max_display_width();
        self.stop_animation();
        let folding = folding(&diff, cx);
        self.mode
            .replace_diff(Rc::new(diff), Rc::new(folding), self.unified, vp);
        self.shared.clear();
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
        self.update_markers();
        cx.notify();
    }

    /// `v`: switches between Split and Unified for all files. The cursor stays on the same line and
    /// height.
    pub(crate) fn toggle_view(&mut self, _: &ToggleView, _: &mut Window, cx: &mut Context<Self>) {
        // Added and deleted files are always shown Unified, so `v` does nothing there
        if !self.mode.diff().can_split() {
            return;
        }
        self.unified = !self.unified;
        self.stop_animation();
        let diff = self.mode.diff().clone();
        let folding = Rc::new(folding(&diff, cx));
        match self.viewport().filter(|_| self.initialized) {
            Some(vp) => self.mode.toggle(folding, vp),
            // Nothing is shown yet, so `step` lands on the first hunk
            None => self.mode = DiffMode::new(diff, folding, self.unified),
        }
        self.update_markers();
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
        self.show_message(if self.unified { "Unified" } else { "Split" }, cx);
        cx.notify();
    }

    /// The text width differs between Split and Unified, and the content may have become shorter.
    fn clamp_scroll_x(&mut self) {
        let max_x = self.max_scroll_x();
        let scroll = self.mode.scroll_mut();
        scroll.scroll_x_to(scroll.x, max_x);
    }

    fn viewport(&self) -> Option<Viewport> {
        let style = self.style.as_ref()?;
        Some(Viewport {
            height: self.viewport_height,
            line_height: style.line_height(),
        })
    }

    fn ensure_style(&mut self, window: &mut Window) {
        if self
            .style
            .as_ref()
            .is_some_and(|s| s.font_size == px(self.font_size))
        {
            return;
        }
        let style = TextStyle::new(self.font_size, window);
        self.shared.set_style(&style);
        self.style = Some(style);
        self.update_markers();
    }

    /// The hunk minimap follows visual lines, so it must be recomputed when the folds change. Unified
    /// computes its marks when the hunk minimap is drawn.
    fn update_markers(&mut self) {
        self.shared.clear_stripes();
        let (Some(style), DiffMode::Split(state)) = (&self.style, &self.mode) else {
            return;
        };
        let lh = style.line_height();
        let (diff, folding) = (&state.diff, &state.folding);
        self.markers = Rc::new(SIDES.map(|s| stripe_markers(diff, folding, s, lh)));
    }

    fn horizontal(&self) -> Horizontal {
        let (Some(bounds), Some(style)) = (self.shared.bounds.get(), self.style.as_ref()) else {
            return Horizontal::default();
        };
        Horizontal {
            text_width: code_viewer::text_width(&self.mode, style, bounds),
            char_width: style.char_width,
            max_x: self.max_scroll_x(),
        }
    }

    /// Finishes an animation where it is, before anything else changes the scroll.
    fn stop_animation(&mut self) {
        if let Some(shown) = self.scroll.stop() {
            *self.mode.scroll_mut() = shown;
        }
    }

    fn execute(&mut self, cmd: Command, window: &mut Window, cx: &mut Context<Self>) {
        let size = match cmd {
            Command::ZoomIn => Some(zoomed(self.font_size, 1.)),
            Command::ZoomOut => Some(zoomed(self.font_size, -1.)),
            Command::ResetZoom => Some(cx.global::<Settings>().font_size),
            Command::Rollback => {
                self.rollback(cx);
                return;
            }
            Command::CopyLocation => {
                if let Some(location) = location(self.mode.location(), &self.paths) {
                    cx.write_to_clipboard(ClipboardItem::new_string(location.clone()));
                    self.show_message(format!("Copied {location}"), cx);
                }
                return;
            }
            _ => None,
        };
        if let Some(size) = size {
            self.zoom(size, window);
            cx.emit(Zoomed {
                font_size: self.font_size,
            });
            cx.notify();
            return;
        }
        let Some(vp) = self.viewport() else {
            return;
        };
        let out = self.mode.execute(cmd, vp, self.horizontal());
        if let (Command::ToggleFolds, true, DiffMode::Split(state)) = (cmd, out.changed, &self.mode)
        {
            let closed = state.folding.visible(Side::Left).next().is_some();
            self.show_message(
                if closed {
                    "Folds closed"
                } else {
                    "Folds opened"
                },
                cx,
            );
        }
        if !out.changed {
            return;
        }
        if out.folding_changed {
            self.update_markers();
        }
        if out.animate {
            self.scroll.animate(self.mode.scroll());
        } else {
            self.scroll.jump(self.mode.scroll());
        }
        cx.notify();
    }

    /// Checks that the hunk can be rolled back and opens the dialog. `on_key_down` sends
    /// `Rollback` on Enter.
    fn rollback(&mut self, cx: &mut Context<Self>) {
        match rollback_request(self.mode.diff(), self.mode.cursor_hunk(), self.can_rollback) {
            Ok(request) => {
                self.overlay = Some(Overlay::Confirm(Rollback(request)));
                cx.notify();
            }
            Err(text) => self.show_message(text, cx),
        }
    }

    /// Comes as an action, so `?` also works when the tree has focus. Cancels the rollback, which
    /// otherwise doesn't see the key.
    pub(crate) fn toggle_help(&mut self, _: &ToggleHelp, _: &mut Window, cx: &mut Context<Self>) {
        self.overlay = match self.overlay {
            Some(Overlay::Help) => None,
            _ => Some(Overlay::Help),
        };
        cx.notify();
    }

    /// Esc passes through when help is not shown, so the rollback dialog gets it.
    pub(crate) fn hide_help(&mut self, _: &HideHelp, _: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.overlay, Some(Overlay::Help)) {
            cx.propagate();
            return;
        }
        self.overlay = None;
        cx.notify();
    }

    fn show_message(&self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        self.message
            .update(cx, |m, cx| m.show(text, Level::Info, cx));
    }

    /// `EditorFontSizeAction`: new font size, and the cursor line is kept at the same place.
    fn zoom(&mut self, size: f32, window: &mut Window) {
        if size == self.font_size {
            return;
        }
        let Some(old) = self.style.as_ref().map(|s| (s.line_height(), s.char_width)) else {
            self.font_size = size;
            return;
        };
        self.font_size = size;
        self.ensure_style(window);
        let Some(vp) = self.viewport() else {
            return;
        };
        let char_width = self.style.as_ref().map_or(old.1, |s| s.char_width);
        self.mode.rescale(old.0, vp, char_width / old.1);
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // While the rollback waits for an answer, no keys go to the diff
        match self.overlay.take() {
            Some(Overlay::Confirm(rollback)) => {
                let k = &ev.keystroke;
                if k.key == "enter" && !k.modifiers.modified() {
                    cx.emit(rollback);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
            other => self.overlay = other,
        }
        match self.keys.feed(to_key(&ev.keystroke)) {
            Parsed::Command(cmd) => {
                self.execute(cmd, window, cx);
                cx.stop_propagation();
            }
            // macOS swallows ctrl keys that don't stop propagation (`handle_key_equivalent`)
            Parsed::Pending => cx.stop_propagation(),
            Parsed::Ignored => {}
        }
    }

    fn hit(&self, position: Point<Pixels>) -> Option<Hit> {
        let (bounds, style) = (self.shared.bounds.get()?, self.style.as_ref()?);
        code_viewer::hit(&self.mode, style, bounds, self.scroll.shown().y, position)
    }

    /// Only Split has folds.
    fn fold_at(&self, position: Point<Pixels>) -> Option<FoldId> {
        let (Some(Hit::Split(side, line)), DiffMode::Split(state)) =
            (self.hit(position), &self.mode)
        else {
            return None;
        };
        state.folding.fold_at_line(side, line)
    }

    /// `FoldingModelSupport.MyMouseListener`: hover applies to the fold block on both sides.
    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.fold_at(ev.position);
        if let DiffMode::Split(state) = &mut self.mode
            && state.folding.hovered() != id
        {
            Rc::make_mut(&mut state.folding).set_hovered(id);
            cx.notify();
        }
    }

    fn max_scroll_x(&self) -> f32 {
        let (Some(bounds), Some(style)) = (self.shared.bounds.get(), self.style.as_ref()) else {
            return 0.;
        };
        let content = (self.max_cols + 1) as f32 * style.char_width;
        content.max(self.shared.widest.get()) - code_viewer::text_width(&self.mode, style, bounds)
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(vp) = self.viewport() else {
            return;
        };
        let delta = ev.delta.pixel_delta(px(vp.line_height));
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        self.stop_animation();
        if dy != 0. {
            let bounds = self.shared.bounds.get();
            code_viewer::wheel(&mut self.mode, bounds, ev.position.x, -dy, vp);
        }
        if dx != 0. {
            let max_x = self.max_scroll_x();
            let scroll = self.mode.scroll_mut();
            scroll.scroll_x_to(scroll.x - dx, max_x);
        }
        self.scroll.jump(self.mode.scroll());
        cx.notify();
    }

    /// A click gives focus and moves the cursor to the line. A click on a fold opens it on both sides
    /// (`DiffLineSeparatorRenderer.doAction`).
    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.keys.reset();
        self.overlay = None;
        let Some(hit) = self.hit(ev.position) else {
            return;
        };
        self.stop_animation();
        match (hit, &mut self.mode) {
            (Hit::Split(side, line), DiffMode::Split(state)) => {
                state.click(side, line);
                if state.folding.fold_at_line(side, line).is_some() {
                    self.execute(Command::FoldOpen, window, cx);
                }
            }
            (Hit::Unified(row), DiffMode::Unified(state)) => state.click(row),
            _ => {}
        }
        self.scroll.jump(self.mode.scroll());
        cx.notify();
    }

    pub(crate) fn zoom_in(&mut self, _: &ZoomIn, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::ZoomIn, w, cx);
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::ZoomOut, w, cx);
    }

    pub(crate) fn reset_zoom(&mut self, _: &ResetZoom, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::ResetZoom, w, cx);
    }

    fn step(&mut self, window: &mut Window) {
        let Some(vp) = self.viewport() else {
            return;
        };
        if !self.initialized {
            self.initialized = true;
            // `InitialScrollPositionSupport` falls back to the first hunk
            self.mode.init(vp);
            self.scroll.jump(self.mode.scroll());
        }
        if self.scroll.step() {
            window.request_animation_frame();
        }
    }

    /// New viewport height: the target is computed for the new height, and the animation jumps there.
    fn resize(&mut self, height: f32) {
        if height == self.viewport_height {
            return;
        }
        self.viewport_height = height;
        let Some(vp) = self.viewport() else {
            return;
        };
        self.mode.resize(vp);
        self.scroll.jump(self.mode.scroll());
    }
}

fn diff_mode(diff: FileDiff, unified: bool, cx: &App) -> DiffMode {
    let folding = folding(&diff, cx);
    DiffMode::new(Rc::new(diff), Rc::new(folding), unified)
}

fn folding(diff: &FileDiff, cx: &App) -> Folding {
    let mut folding = Folding::new(diff);
    folding.expand_all(!cx.global::<Settings>().collapse_unchanged);
    folding
}

impl EventEmitter<Zoomed> for FileDiffView {}
impl EventEmitter<Rollback> for FileDiffView {}

impl Focusable for FileDiffView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileDiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_style(window);
        // The keymap bar can wrap over several lines, so the height comes from the previous layout
        let height = self
            .shared
            .bounds
            .get()
            .map_or(f32::from(window.viewport_size().height) - BAR_HEIGHT, |b| {
                f32::from(b.size.height)
            });
        self.resize(height);
        self.step(window);

        let theme = cx.global::<Settings>().theme;
        let focused = self.focus.is_focused(window) && window.is_window_active();
        let painter = self.style.clone().map(|style| {
            let shown = self.scroll.shown();
            let scale = window.scale_factor();
            Painter::new(
                &self.mode,
                &self.markers,
                style,
                shown,
                focused,
                scale,
                theme,
            )
        });
        let hovered =
            matches!(&self.mode, DiffMode::Split(state) if state.folding.hovered().is_some());
        let shared = self.shared.clone();

        let keys = KeyHelp { theme };
        let footer = div()
            .relative()
            .flex_none()
            .child(keys.bar())
            .child(self.message.clone());
        let overlay = self.overlay.as_ref().map(|overlay| match overlay {
            Overlay::Help => keys.popup(),
            Overlay::Confirm(_) => rollback_dialog(&theme),
        });

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.editor_bg)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::reset_zoom))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::hide_help))
            .on_action(cx.listener(Self::toggle_view))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(hovered, |d| d.cursor_pointer())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        if let Some(painter) = &painter {
                            painter.paint(bounds, &shared, window, cx);
                        }
                    },
                )
                .flex_1()
                .w_full(),
            )
            .child(footer)
            .children(overlay)
    }
}
