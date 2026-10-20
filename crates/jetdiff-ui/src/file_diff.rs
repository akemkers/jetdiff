//! To editorer med divider mellom. `DiffState` eier cursor, folding og scroll-målet. Her
//! ligger det som trenger GPUI: taster, mus, animasjon mot målet og zoom.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render,
    ScrollWheelEvent, SharedString, Window, actions, canvas, div, prelude::*, px,
};
use jetdiff_core::side::Side;
use jetdiff_view::columns::display_width;
use jetdiff_view::metrics::zoomed;
use jetdiff_view::{
    Command, DiffState, FileDiff, FoldId, Folding, Horizontal, KeyParser, Parsed, StripeMarker,
    Viewport, stripe_markers,
};

use crate::editor_paint::StripeRects;
use crate::help::{BAR_HEIGHT, HideHelp, KeyHelp, ToggleHelp};
use crate::keystroke::to_key;
use crate::message::{Level, Message};
use crate::rollback_dialog::rollback_dialog;
use crate::scroll_animator::ScrollAnimator;
use crate::settings::Settings;
use crate::side_editor::{CaretRow, SideCache};
use crate::split::{self, SplitPainter};
use crate::text::TextStyle;
use crate::theme::UiTheme;

actions!(jetdiff, [ZoomIn, ZoomOut, ResetZoom]);

/// Linjer utenfor viewporten som beholdes shapet per side.
const CACHE_CAPACITY: usize = 2000;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

fn idx(side: Side) -> usize {
    side.select(0, 1)
}

/// Brukeren vil rulle tilbake en endring. Sendes til den som kan skrive fila.
pub struct Rollback {
    /// Høyre side slik den vises, for å oppdage om fila er endret siden.
    pub before: String,
    /// Linjene i endringen, per side.
    pub lines: [Range<usize>; 2],
}

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

/// Det paint trenger å dele med hendelsene: siste bounds og shapede linjer.
struct Shared {
    bounds: Cell<Option<Bounds<Pixels>>>,
    caches: RefCell<[SideCache; 2]>,
    stripes: RefCell<[StripeRects; 2]>,
    widest: Cell<f32>,
}

pub struct FileDiffView {
    state: DiffState,
    font_size: f32,
    style: Option<TextStyle>,
    /// Det som vises nå. Målet er `state.scroll`.
    scroll: ScrollAnimator,
    keys: KeyParser,
    initialized: bool,
    viewport_height: f32,
    markers: Rc<[Vec<StripeMarker>; 2]>,
    /// Lengste linje i tegn, regnet én gang. Gir maks horisontal scroll uten å shape alt.
    max_cols: usize,
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
        Self {
            max_cols: max_cols(&diff),
            state: diff_state(diff, cx),
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
            shared: Rc::new(Shared {
                bounds: Cell::new(None),
                caches: RefCell::new(new_caches()),
                stripes: RefCell::new([StripeRects::default(), StripeRects::default()]),
                widest: Cell::new(0.),
            }),
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
        self.state = diff_state(diff, cx);
        self.scroll = ScrollAnimator::default();
        self.keys.reset();
        self.initialized = false;
        *self.shared.caches.borrow_mut() = new_caches();
        self.shared.widest.set(0.);
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
        self.state.replace_diff(Rc::new(diff), Rc::new(folding), vp);
        *self.shared.caches.borrow_mut() = new_caches();
        self.shared.widest.set(0.);
        let max_x = self.max_scroll_x();
        self.state.scroll.scroll_x_to(self.state.scroll.x, max_x);
        self.scroll.jump(self.state.scroll);
        self.update_markers();
        cx.notify();
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
        // Nøkkelen har fontstørrelsen, så zoom tømmer cachen
        let mut caches = self.shared.caches.borrow_mut();
        for cache in caches.iter_mut() {
            cache.set_key(style.cache_key());
        }
        drop(caches);
        self.shared.widest.set(0.);
        self.style = Some(style);
        self.update_markers();
    }

    /// Stripen følger visuelle linjer, så den må regnes på nytt når foldene endres.
    fn update_markers(&mut self) {
        let Some(style) = &self.style else {
            return;
        };
        let lh = style.line_height();
        let (diff, folding) = (&self.state.diff, &self.state.folding);
        self.markers = Rc::new(SIDES.map(|s| stripe_markers(diff, folding, s, lh)));
        *self.shared.stripes.borrow_mut() = Default::default();
    }

    fn horizontal(&self) -> Horizontal {
        let (Some(bounds), Some(style)) = (self.shared.bounds.get(), self.style.as_ref()) else {
            return Horizontal::default();
        };
        Horizontal {
            text_width: split::text_width(&self.state.diff, style, bounds),
            char_width: style.char_width,
            max_x: self.max_scroll_x(),
        }
    }

    /// Fullfører en animasjon der den står, før noe annet endrer scroll.
    fn stop_animation(&mut self) {
        if let Some(shown) = self.scroll.stop() {
            self.state.scroll = shown;
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
                let c = self.state.location();
                let path = &self.paths[idx(c.side)];
                if !path.is_empty() {
                    let location = format!("{path}:{}", c.line + 1);
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
        let out = self.state.execute(cmd, vp, self.horizontal());
        if cmd == Command::ToggleFolds && out.changed {
            let closed = self.state.folding.visible(Side::Left).next().is_some();
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
            self.scroll.animate(self.state.scroll);
        } else {
            self.scroll.jump(self.state.scroll);
        }
        cx.notify();
    }

    /// Sjekker at endringen kan rulles tilbake og åpner dialogen. `on_key_down` sender
    /// `Rollback` på Enter.
    fn rollback(&mut self, cx: &mut Context<Self>) {
        let diff = &self.state.diff;
        if !self.can_rollback {
            self.show_message("Bare lokale endringer kan rulles tilbake", cx);
            return;
        }
        if diff.single_side().is_some() {
            self.show_message("Hele fila er ny eller slettet", cx);
            return;
        }
        let Some(block) = self.state.cursor_block() else {
            self.show_message("Ingen endring på linja", cx);
            return;
        };
        self.overlay = Some(Overlay::Confirm(Rollback {
            before: diff.text(Side::Right).to_owned(),
            lines: diff.blocks()[block].lines.clone(),
        }));
        cx.notify();
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
        self.state.rescale(old.0, vp, char_width / old.1);
        let max_x = self.max_scroll_x();
        self.state.scroll.scroll_x_to(self.state.scroll.x, max_x);
        self.scroll.jump(self.state.scroll);
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

    fn hit(&self, position: Point<Pixels>) -> Option<(Side, usize)> {
        let (bounds, style) = (self.shared.bounds.get()?, self.style.as_ref()?);
        let state = &self.state;
        split::hit(
            &state.diff,
            &state.folding,
            style,
            bounds,
            self.scroll.shown().y,
            position,
        )
    }

    fn fold_at(&self, position: Point<Pixels>) -> Option<FoldId> {
        let (side, line) = self.hit(position)?;
        self.state.folding.fold_at_line(side, line)
    }

    /// `FoldingModelSupport.MyMouseListener`: hover gjelder blokken på begge sider.
    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.fold_at(ev.position);
        if self.state.folding.hovered() != id {
            Rc::make_mut(&mut self.state.folding).set_hovered(id);
            cx.notify();
        }
    }

    fn side_at(&self, x: Pixels) -> Side {
        self.shared
            .bounds
            .get()
            .map_or(self.state.master, |bounds| {
                split::side_at(&self.state.diff, bounds, x)
            })
    }

    fn max_scroll_x(&self) -> f32 {
        let (Some(bounds), Some(style)) = (self.shared.bounds.get(), self.style.as_ref()) else {
            return 0.;
        };
        let content = (self.max_cols + 1) as f32 * style.char_width;
        content.max(self.shared.widest.get()) - split::text_width(&self.state.diff, style, bounds)
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(vp) = self.viewport() else {
            return;
        };
        let delta = ev.delta.pixel_delta(px(vp.line_height));
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        self.stop_animation();
        if dy != 0. {
            self.state.wheel(self.side_at(ev.position.x), -dy, vp);
        }
        if dx != 0. {
            let max_x = self.max_scroll_x();
            let x = self.state.scroll.x - dx;
            self.state.scroll.scroll_x_to(x, max_x);
        }
        self.scroll.jump(self.state.scroll);
        cx.notify();
    }

    /// Klikk gir fokus og markerer linja. Klikk på en fold åpner neste nivå på
    /// begge sider (`DiffLineSeparatorRenderer.doAction`).
    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.keys.reset();
        self.overlay = None;
        let Some((side, line)) = self.hit(ev.position) else {
            return;
        };
        self.stop_animation();
        self.state.click(side, line);
        if self.state.folding.fold_at_line(side, line).is_some() {
            self.execute(Command::FoldOpen, window, cx);
            // Musa står fortsatt på linja, som nå kan være neste nivå
            let id = self.fold_at(ev.position);
            Rc::make_mut(&mut self.state.folding).set_hovered(id);
        }
        self.scroll.jump(self.state.scroll);
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
            self.state.init(vp);
            self.scroll.jump(self.state.scroll);
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
        self.state.resize(vp);
        self.scroll.jump(self.state.scroll);
    }

    /// `None` før stilen er laget.
    fn split_painter(&self, focused: bool, scale: f32, theme: UiTheme) -> Option<SplitPainter> {
        let style = self.style.clone()?;
        // Den andre siden viser overført linje svakere, så man ser hvor H og L lander.
        // IntelliJ har caret row bare på aktiv side.
        let shown = self.scroll.shown();
        let cursor = self.state.cursor;
        let other = self.state.transferred_cursor();
        let carets = SIDES.map(|side| {
            let caret = if side == cursor.side { cursor } else { other };
            CaretRow {
                line: caret.line,
                active: focused && side == cursor.side,
            }
        });
        Some(SplitPainter {
            diff: self.state.diff.clone(),
            folding: self.state.folding.clone(),
            theme,
            style,
            markers: self.markers.clone(),
            scroll_y: shown.y.map(|y| snap(y, scale)),
            scroll_x: snap(shown.x, scale),
            carets,
        })
    }
}

fn diff_state(diff: FileDiff, cx: &App) -> DiffState {
    let folding = folding(&diff, cx);
    DiffState::new(Rc::new(diff), Rc::new(folding))
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

fn new_caches() -> [SideCache; 2] {
    [
        SideCache::new(CACHE_CAPACITY),
        SideCache::new(CACHE_CAPACITY),
    ]
}

/// Snapper til hele device-piksler så kanter og tekst ikke blir uskarpe midt i en scroll.
fn snap(y: f32, scale: f32) -> f32 {
    (y * scale).round() / scale
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
        let painter = self.split_painter(focused, window.scale_factor(), theme);
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
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(self.state.folding.hovered().is_some(), |d| {
                d.cursor_pointer()
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        let Some(painter) = painter else {
                            return;
                        };
                        let old = shared.bounds.replace(Some(bounds));
                        if old.map(|old| old.size.height) != Some(bounds.size.height) {
                            window.refresh();
                        }
                        let widest = painter.paint(
                            bounds,
                            &mut shared.caches.borrow_mut(),
                            &mut shared.stripes.borrow_mut(),
                            window,
                            cx,
                        );
                        shared.widest.set(shared.widest.get().max(widest));
                    },
                )
                .flex_1()
                .w_full(),
            )
            .child(footer)
            .children(overlay)
    }
}
