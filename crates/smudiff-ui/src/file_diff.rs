//! To editorer med divider mellom. `DiffState` eier cursor, folding og scroll-målet. Her
//! ligger det som trenger GPUI: taster, mus, animasjon mot målet og zoom.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    Bounds, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Keystroke, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ScrollWheelEvent, Window, actions,
    canvas, div, prelude::*, px,
};
use smudiff_core::side::Side;
use smudiff_view::columns::display_width;
use smudiff_view::metrics::{FONT_SIZE, zoomed};
use smudiff_view::{
    Command, DiffState, FileDiff, FoldId, FoldSettings, Folding, Horizontal, Key, KeyCode,
    KeyParser, Parsed, ScrollAnimation, ScrollState, SideGeometry, StripeMarker, Viewport,
    stripe_markers,
};

use crate::side_editor::{CaretRow, SideCache, SideEditor, StripeRects};
use crate::text::TextStyle;
use crate::theme::UiTheme;

actions!(smudiff, [NextContextRange, ZoomIn, ZoomOut, ResetZoom]);

/// Varighet på animert hopp til endring.
const NAV_ANIMATION_MS: f32 = 170.;

/// Høyden på tastelinja nederst.
const HELP_HEIGHT: f32 = 24.;

const HELP: [(&str, &str); 9] = [
    ("j k", "opp/ned"),
    ("h l", "sideveis"),
    ("d u", "scroll side"),
    ("n p", "hopp til endring"),
    ("o", "åpne fold"),
    ("f", "toggle folds"),
    ("H L", "venstre/høyre side"),
    ("gg G", "topp og bunn"),
    ("q", "avslutt"),
];

/// Vises først når diffen er en del av et filtre.
const FILE_HELP: (&str, &str) = ("J K", "neste/forrige fil");

/// Linjer utenfor viewporten som beholdes shapet per side.
const CACHE_CAPACITY: usize = 2000;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

fn idx(side: Side) -> usize {
    side.select(0, 1)
}

/// `J`/`K`. Den som eier filtreet bytter fil.
pub struct StepFile {
    pub forward: bool,
}

/// Ny fontstørrelse etter zoom, så resten av vinduet kan følge med.
pub struct Zoomed {
    pub font_size: f32,
}

struct Animation {
    anim: ScrollAnimation,
    start: Instant,
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
    theme: UiTheme,
    family: &'static str,
    font_size: f32,
    style: Option<TextStyle>,
    /// Det som vises nå. Under en animasjon ligger den etter `state.scroll`, som er målet.
    shown: ScrollState,
    animation: Option<Animation>,
    keys: KeyParser,
    initialized: bool,
    viewport_height: f32,
    markers: Rc<[Vec<StripeMarker>; 2]>,
    /// Lengste linje i tegn, regnet én gang. Gir maks horisontal scroll uten å shape alt.
    max_cols: usize,
    file_keys: bool,
    shared: Rc<Shared>,
    focus: FocusHandle,
}

/// GPUI-tast til vår `Key`. `key_char` har shift og alt med, så `G`, `$` og `[` på norsk
/// tastatur kommer riktig. Med ctrl er `key_char` tom, og `key` er grunntasten.
fn to_key(k: &Keystroke) -> Key {
    let m = k.modifiers;
    let single = |s: &str| {
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        }
    };
    let code = match k.key.as_str() {
        "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        f if let Some(n) = f.strip_prefix('f').and_then(|n| n.parse().ok()) => KeyCode::F(n),
        key => {
            let typed = if m.control {
                None
            } else {
                k.key_char.as_deref().and_then(single)
            };
            match typed.or_else(|| single(key)) {
                Some(c) => KeyCode::Char(c),
                None => KeyCode::Other,
            }
        }
    };
    Key {
        code,
        ctrl: m.control,
        shift: m.shift,
        cmd: m.platform,
    }
}

impl FileDiffView {
    pub fn new(
        diff: FileDiff,
        theme: UiTheme,
        family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            max_cols: max_cols(&diff),
            state: diff_state(diff),
            theme,
            family,
            font_size: FONT_SIZE,
            style: None,
            shown: ScrollState::default(),
            animation: None,
            keys: KeyParser::default(),
            initialized: false,
            viewport_height: 0.,
            markers: Rc::new([Vec::new(), Vec::new()]),
            file_keys: false,
            shared: Rc::new(Shared {
                bounds: Cell::new(None),
                caches: RefCell::new(new_caches()),
                stripes: RefCell::new([StripeRects::default(), StripeRects::default()]),
                widest: Cell::new(0.),
            }),
            focus: cx.focus_handle(),
        }
    }

    pub fn with_file_keys(mut self) -> Self {
        self.file_keys = true;
        self
    }

    /// Ny fil i samme visning. Zoom beholdes, cursor og scroll starter på nytt.
    pub fn set_diff(&mut self, diff: FileDiff, cx: &mut Context<Self>) {
        self.max_cols = max_cols(&diff);
        self.state = diff_state(diff);
        self.shown = ScrollState::default();
        self.animation = None;
        self.keys.reset();
        self.initialized = false;
        *self.shared.caches.borrow_mut() = new_caches();
        self.shared.widest.set(0.);
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
            .is_some_and(|s| s.font.family == self.family && s.font_size == px(self.font_size))
        {
            return;
        }
        let style = TextStyle::new(self.family, self.font_size, window);
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
            text_width: self.text_width(style, bounds),
            char_width: style.char_width,
            max_x: self.max_scroll_x(),
        }
    }

    /// Fullfører en animasjon der den står, før noe annet endrer scroll.
    fn stop_animation(&mut self) {
        if self.animation.take().is_some() {
            self.state.scroll = self.shown;
        }
    }

    fn execute(&mut self, cmd: Command, window: &mut Window, cx: &mut Context<Self>) {
        let size = match cmd {
            Command::ZoomIn => Some(zoomed(self.font_size, 1.)),
            Command::ZoomOut => Some(zoomed(self.font_size, -1.)),
            Command::ResetZoom => Some(FONT_SIZE),
            Command::Quit => {
                cx.quit();
                return;
            }
            Command::NextFile | Command::PrevFile => {
                cx.emit(StepFile {
                    forward: cmd == Command::NextFile,
                });
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
        if !out.changed {
            return;
        }
        if out.folding_changed {
            self.update_markers();
        }
        if out.animate {
            self.animate();
        } else {
            self.animation = None;
            self.shown = self.state.scroll;
        }
        cx.notify();
    }

    /// Fra det som vises til målet. Midt i en animasjon starter den nye der den gamle står.
    fn animate(&mut self) {
        if self.shown == self.state.scroll {
            self.animation = None;
            return;
        }
        self.animation = Some(Animation {
            anim: ScrollAnimation::new(self.shown, self.state.scroll, NAV_ANIMATION_MS),
            start: Instant::now(),
        });
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
        self.animation = None;
        self.font_size = size;
        self.ensure_style(window);
        let Some(vp) = self.viewport() else {
            return;
        };
        let char_width = self.style.as_ref().map_or(old.1, |s| s.char_width);
        self.state.rescale(old.0, vp, char_width / old.1);
        let max_x = self.max_scroll_x();
        self.state.scroll.scroll_x_to(self.state.scroll.x, max_x);
        self.shown = self.state.scroll;
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    /// Side og logisk linje under musa, `None` i divideren.
    fn hit(&self, position: Point<Pixels>) -> Option<(Side, usize)> {
        let (vp, bounds, style) = (
            self.viewport()?,
            self.shared.bounds.get()?,
            self.style.as_ref()?,
        );
        let geoms = side_geometry(&self.state.diff, style, bounds);
        let x = f32::from(position.x);
        let side = SIDES
            .into_iter()
            .find(|&s| geoms[idx(s)].bounds().contains(&x))?;
        let layout = self.state.folding.layout(side, vp.line_height);
        let y = f32::from(position.y - bounds.origin.y) + self.shown.y(side);
        let line = layout.y_to_logical_line(y).min(layout.line_count() - 1);
        Some((side, line))
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
        if let Some(side) = self.state.diff.single_side() {
            return side;
        }
        let Some(bounds) = self.shared.bounds.get() else {
            return self.state.master;
        };
        if x < bounds.center().x {
            Side::Left
        } else {
            Side::Right
        }
    }

    fn max_scroll_x(&self) -> f32 {
        let (Some(bounds), Some(style)) = (self.shared.bounds.get(), self.style.as_ref()) else {
            return 0.;
        };
        let content = (self.max_cols + 1) as f32 * style.char_width;
        content.max(self.shared.widest.get()) - self.text_width(style, bounds)
    }

    /// Sidene er like brede, så venstre side holder når begge vises.
    fn text_width(&self, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
        let side = self.state.diff.single_side().unwrap_or(Side::Left);
        let g = &side_geometry(&self.state.diff, style, bounds)[idx(side)];
        g.text.end - g.text_x
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
        self.shown = self.state.scroll;
        cx.notify();
    }

    /// Klikk gir fokus og markerer linja. Klikk på en fold åpner neste nivå på
    /// begge sider (`DiffLineSeparatorRenderer.doAction`).
    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.keys.reset();
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
        self.shown = self.state.scroll;
        cx.notify();
    }

    /// `TextDiffSettings.setContextRange`: foldene bygges på nytt med tilstanden bevart.
    fn next_context_range(&mut self, _: &NextContextRange, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::NextContextRange, w, cx);
    }

    fn zoom_in(&mut self, _: &ZoomIn, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::ZoomIn, w, cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, w: &mut Window, cx: &mut Context<Self>) {
        self.execute(Command::ZoomOut, w, cx);
    }

    fn reset_zoom(&mut self, _: &ResetZoom, w: &mut Window, cx: &mut Context<Self>) {
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
            self.shown = self.state.scroll;
        }
        if let Some(a) = &self.animation {
            let elapsed = a.start.elapsed().as_secs_f32() * 1000.;
            let (state, done) = a.anim.sample(elapsed);
            self.shown = state;
            if done {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
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
        self.animation = None;
        self.state.resize(vp);
        self.shown = self.state.scroll;
    }
}

fn diff_state(diff: FileDiff) -> DiffState {
    let folding = Folding::new(&diff, FoldSettings::default());
    DiffState::new(Rc::new(diff), Rc::new(folding))
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

fn side_geometry(diff: &FileDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> [SideGeometry; 2] {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    smudiff_view::side_geometry(diff, style.number_char_width, x, width)
}

/// Snapper til hele device-piksler så kanter og tekst ikke blir uskarpe midt i en scroll.
fn snap(y: f32, scale: f32) -> f32 {
    (y * scale).round() / scale
}

impl EventEmitter<StepFile> for FileDiffView {}
impl EventEmitter<Zoomed> for FileDiffView {}

impl Focusable for FileDiffView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileDiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_style(window);
        // Tastelinja kan bryte over flere linjer, så høyden kommer fra forrige layout
        let height = self.shared.bounds.get().map_or(
            f32::from(window.viewport_size().height) - HELP_HEIGHT,
            |b| f32::from(b.size.height),
        );
        self.resize(height);
        self.step(window);

        let scale = window.scale_factor();
        let scroll_y = self.shown.y.map(|y| snap(y, scale));
        let scroll_x = snap(self.shown.x, scale);
        let diff = self.state.diff.clone();
        let folding = self.state.folding.clone();
        let theme = self.theme;
        let style = self.style.clone();
        let markers = self.markers.clone();
        let shared = self.shared.clone();

        // Den andre siden viser overført linje svakere, så man ser hvor H og L lander.
        // IntelliJ har caret row bare på aktiv side.
        let focused = self.focus.is_focused(window) && window.is_window_active();
        let cursor = self.state.cursor;
        let other = self.state.transferred_cursor();
        let carets = SIDES.map(|s| {
            let c = if s == cursor.side { cursor } else { other };
            CaretRow {
                line: c.line,
                active: focused && s == cursor.side,
            }
        });

        let single = self.state.diff.single_side().is_some();
        let help = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .min_h(px(HELP_HEIGHT))
            .px(px(10.))
            .py(px(4.))
            .gap_x(px(16.))
            .gap_y(px(2.))
            .items_center()
            .bg(theme.gutter_bg)
            .font_family(self.family)
            .text_size(px(12.))
            .text_color(theme.line_number)
            .children(
                self.file_keys
                    .then_some(&FILE_HELP)
                    .into_iter()
                    .chain(&HELP)
                    .filter(|(keys, _)| !(single && *keys == "H L"))
                    .map(|(keys, what)| {
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(div().text_color(theme.editor_fg).child(*keys))
                            .child(*what)
                    }),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.editor_bg)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::next_context_range))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::reset_zoom))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(folding.hovered().is_some(), |d| d.cursor_pointer())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        let Some(style) = style else {
                            return;
                        };
                        let old = shared.bounds.replace(Some(bounds));
                        if old.map(|b| b.size.height) != Some(bounds.size.height) {
                            window.refresh();
                        }
                        let top = f32::from(bounds.origin.y);
                        let height = f32::from(bounds.size.height);
                        let geoms = side_geometry(&diff, &style, bounds);
                        let mut caches = shared.caches.borrow_mut();
                        let mut stripes = shared.stripes.borrow_mut();
                        let mut widest = shared.widest.get();
                        let single = diff.single_side();
                        for (i, geom) in geoms.iter().enumerate() {
                            let side = SIDES[i];
                            if single.is_some_and(|s| s != side) {
                                continue;
                            }
                            if stripes[i].track_height != height {
                                let content =
                                    folding.layout(side, style.line_height()).content_height();
                                stripes[i] = StripeRects::new(&markers[i], content, height, &theme);
                            }
                            let editor = SideEditor {
                                diff: &diff,
                                folding: &folding,
                                geom,
                                theme: &theme,
                                style: &style,
                                top,
                                height,
                                scroll_y: scroll_y[i],
                                scroll_x,
                                caret: carets[i],
                                stripe: &stripes[i],
                            };
                            widest = widest.max(editor.paint(&mut caches[i], window, cx));
                        }
                        shared.widest.set(widest);
                        if single.is_some() {
                            return;
                        }
                        crate::divider::Divider {
                            diff: &diff,
                            folding: &folding,
                            theme: &theme,
                            x: geoms[0].gutter.end,
                            top,
                            viewport: Viewport {
                                height,
                                line_height: style.line_height(),
                            },
                            scroll_y,
                        }
                        .paint(window);
                    },
                )
                .flex_1()
                .w_full(),
            )
            .child(help)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn stroke(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.into(),
            key_char: key_char.map(Into::into),
        }
    }

    #[test]
    fn taster_fra_gpui() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..none
        };
        let ctrl = Modifiers {
            control: true,
            ..none
        };
        let alt = Modifiers { alt: true, ..none };
        assert_eq!(
            to_key(&stroke("g", Some("G"), shift)).code,
            KeyCode::Char('G')
        );
        assert_eq!(to_key(&stroke("d", None, ctrl)), Key::ctrl('d'));
        // Norsk Alt+8 gir `[`
        assert_eq!(
            to_key(&stroke("8", Some("["), alt)).code,
            KeyCode::Char('[')
        );
        assert_eq!(to_key(&stroke("f7", None, shift)).code, KeyCode::F(7));
        assert_eq!(to_key(&stroke("escape", None, none)).code, KeyCode::Esc);
        assert_eq!(
            to_key(&stroke("f", Some("f"), none)).code,
            KeyCode::Char('f')
        );
        assert_eq!(to_key(&stroke("tab", None, none)).code, KeyCode::Tab);
        assert_eq!(to_key(&stroke("home", None, none)).code, KeyCode::Other);
        let cmd = to_key(&stroke(
            "=",
            None,
            Modifiers {
                platform: true,
                ..none
            },
        ));
        assert!(cmd.cmd);
    }
}
