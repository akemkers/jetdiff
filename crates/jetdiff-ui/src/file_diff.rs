//! Diffen av én fil, split eller unified. `DiffMode` eier cursor, folding og scroll-målet.
//! Her ligger det som trenger GPUI: taster, mus, animasjon mot målet og zoom.

use std::rc::Rc;

use gpui::{
    App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ScrollWheelEvent,
    SharedString, Window, actions, canvas, div, prelude::*, px,
};
use jetdiff_app::{RollbackRequest, location, rollback_request};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::columns::display_width;
use jetdiff_viewmodel::metrics::zoomed;
use jetdiff_viewmodel::{
    Command, DiffMode, FileDiff, FoldId, Folding, Horizontal, KeyParser, Parsed, StripeMarker,
    Viewport, stripe_markers,
};

use crate::help::{BAR_HEIGHT, HideHelp, KeyHelp, ToggleHelp};
use crate::keystroke::to_key;
use crate::message::{Level, Message};
use crate::painter::{self, Hit, Painter, Shared};
use crate::rollback_dialog::rollback_dialog;
use crate::scroll_animator::ScrollAnimator;
use crate::settings::Settings;
use crate::text::TextStyle;

actions!(jetdiff, [ZoomIn, ZoomOut, ResetZoom, ToggleView]);

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// Brukeren vil rulle tilbake en endring. Sendes til den som kan skrive fila.
pub struct Rollback(pub RollbackRequest);

/// Ligger over diffen. Bare én om gangen.
enum Overlay {
    /// Venter på bekreftelse av rollback. Enter bekrefter, alt annet avbryter.
    Confirm(Rollback),
    Help,
}

/// Ny fontstørrelse etter zoom, så resten av vinduet kan følge med.
pub struct Zoomed {
    pub font_size: f32,
}

pub struct FileDiffView {
    mode: DiffMode,
    font_size: f32,
    style: Option<TextStyle>,
    /// Det som vises nå. Målet er `mode.scroll()`.
    scroll: ScrollAnimator,
    keys: KeyParser,
    initialized: bool,
    viewport_height: f32,
    markers: Rc<[Vec<StripeMarker>; 2]>,
    /// Lengste linje i tegn, regnet én gang. Gir maks horisontal scroll uten å shape alt.
    max_cols: usize,
    /// Split og unified for alle filer, til `v` bytter.
    unified: bool,
    /// Høyre side er en fil i working tree, så endringer kan rulles tilbake.
    can_rollback: bool,
    overlay: Option<Overlay>,
    /// Venstre og høyre sti for `y`. Tom når det ikke finnes noen fil.
    paths: [String; 2],
    /// Eies av den som lager diffen, men tegnes over tastelinja her.
    message: Entity<Message>,
    shared: Rc<Shared>,
    focus: FocusHandle,
}

impl FileDiffView {
    /// Én fil, med egne beskjeder.
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
            max_cols: max_cols(&diff),
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

    /// Ny fil i samme visning. Zoom beholdes, cursor og scroll starter på nytt.
    pub fn set_diff(&mut self, diff: FileDiff, paths: [String; 2], cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Confirm(_))) {
            self.overlay = None;
        }
        self.paths = paths;
        self.max_cols = max_cols(&diff);
        self.mode = diff_mode(diff, self.unified, cx);
        self.scroll = ScrollAnimator::default();
        self.keys.reset();
        self.initialized = false;
        self.shared.clear();
        self.update_markers();
        cx.notify();
    }

    /// Samme fil med nytt innhold, etter rollback eller lagring. Cursor og scroll beholdes.
    pub fn update_diff(&mut self, diff: FileDiff, paths: [String; 2], cx: &mut Context<Self>) {
        let Some(vp) = self.viewport().filter(|_| self.initialized) else {
            self.set_diff(diff, paths, cx);
            return;
        };
        self.paths = paths;
        self.max_cols = max_cols(&diff);
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

    /// `v`: bytter mellom split og unified for alle filer. Cursoren blir på samme linje og
    /// høyde.
    pub(crate) fn toggle_view(&mut self, _: &ToggleView, _: &mut Window, cx: &mut Context<Self>) {
        // Nye og slettede filer vises alltid unified, så `v` gjør ingenting der
        if self.mode.diff().single_side().is_some() {
            return;
        }
        self.unified = !self.unified;
        self.stop_animation();
        let diff = self.mode.diff().clone();
        let folding = Rc::new(folding(&diff, cx));
        match self.viewport().filter(|_| self.initialized) {
            Some(vp) => self.mode.toggle(folding, vp),
            // Ingenting er vist ennå, så `step` lander på første endring
            None => self.mode = DiffMode::new(diff, folding, self.unified),
        }
        self.update_markers();
        self.clamp_scroll_x();
        self.scroll.jump(self.mode.scroll());
        self.show_message(if self.unified { "Unified" } else { "Split" }, cx);
        cx.notify();
    }

    /// Tekstbredden er ulik i split og unified, og innholdet kan ha blitt kortere.
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

    /// Stripen følger visuelle linjer, så den må regnes på nytt når foldene endres. Unified
    /// regner sine markører når stripen tegnes.
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
            text_width: painter::text_width(&self.mode, style, bounds),
            char_width: style.char_width,
            max_x: self.max_scroll_x(),
        }
    }

    /// Fullfører en animasjon der den står, før noe annet endrer scroll.
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
                    self.show_message(format!("Kopierte {location}"), cx);
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
                    "Folds lukket"
                } else {
                    "Folds åpnet"
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

    /// Sjekker at endringen kan rulles tilbake og åpner dialogen. `on_key_down` sender
    /// `Rollback` på Enter.
    fn rollback(&mut self, cx: &mut Context<Self>) {
        match rollback_request(
            self.mode.diff(),
            self.mode.cursor_block(),
            self.can_rollback,
        ) {
            Ok(request) => {
                self.overlay = Some(Overlay::Confirm(Rollback(request)));
                cx.notify();
            }
            Err(text) => self.show_message(text, cx),
        }
    }

    /// Kommer som action, så `?` virker også når treet har fokus. Avbryter rollback, som
    /// ellers ikke ser tasten.
    pub(crate) fn toggle_help(&mut self, _: &ToggleHelp, _: &mut Window, cx: &mut Context<Self>) {
        self.overlay = match self.overlay {
            Some(Overlay::Help) => None,
            _ => Some(Overlay::Help),
        };
        cx.notify();
    }

    /// Esc går videre når hjelpen ikke vises, så rollback-dialogen får den.
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

    /// `EditorFontSizeAction`: ny fontstørrelse, og cursorlinja holdes på samme sted.
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
        // Mens rollback venter på svar, går ingen taster til diffen
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
            // macOS spiser ctrl-taster som ikke stopper propagering (`handle_key_equivalent`)
            Parsed::Pending => cx.stop_propagation(),
            Parsed::Ignored => {}
        }
    }

    fn hit(&self, position: Point<Pixels>) -> Option<Hit> {
        let (bounds, style) = (self.shared.bounds.get()?, self.style.as_ref()?);
        painter::hit(&self.mode, style, bounds, self.scroll.shown().y, position)
    }

    /// Bare split har folds.
    fn fold_at(&self, position: Point<Pixels>) -> Option<FoldId> {
        let (Some(Hit::Split(side, line)), DiffMode::Split(state)) =
            (self.hit(position), &self.mode)
        else {
            return None;
        };
        state.folding.fold_at_line(side, line)
    }

    /// `FoldingModelSupport.MyMouseListener`: hover gjelder blokken på begge sider.
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
        content.max(self.shared.widest.get()) - painter::text_width(&self.mode, style, bounds)
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
            painter::wheel(&mut self.mode, bounds, ev.position.x, -dy, vp);
        }
        if dx != 0. {
            let max_x = self.max_scroll_x();
            let scroll = self.mode.scroll_mut();
            scroll.scroll_x_to(scroll.x - dx, max_x);
        }
        self.scroll.jump(self.mode.scroll());
        cx.notify();
    }

    /// Klikk gir fokus og markerer linja. Klikk på en fold åpner neste nivå på
    /// begge sider (`DiffLineSeparatorRenderer.doAction`).
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
                    // Musa står fortsatt på linja, som nå kan være neste nivå
                    let id = self.fold_at(ev.position);
                    if let DiffMode::Split(state) = &mut self.mode {
                        Rc::make_mut(&mut state.folding).set_hovered(id);
                    }
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
            // `InitialScrollPositionSupport` faller tilbake til første endring
            self.mode.init(vp);
            self.scroll.jump(self.mode.scroll());
        }
        if self.scroll.step() {
            window.request_animation_frame();
        }
    }

    /// Ny viewport-høyde: målet regnes for den nye høyden, og animasjonen hopper dit.
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

fn max_cols(diff: &FileDiff) -> usize {
    SIDES
        .iter()
        .flat_map(|&s| (0..diff.line_count(s)).map(move |l| (s, l)))
        .map(|(s, l)| display_width(diff.line(s, l)))
        .max()
        .unwrap_or(0)
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
        // Tastelinja kan bryte over flere linjer, så høyden kommer fra forrige layout
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
