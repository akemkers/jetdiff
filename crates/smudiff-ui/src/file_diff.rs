//! To editorer med divider mellom. Eier scroll, navigasjon og animasjon, og kobler
//! GPUI-hendelser til `smudiff-view`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    Bounds, Context, FocusHandle, Focusable, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    Point, Render, ScrollWheelEvent, Window, actions, canvas, div, prelude::*, px,
};
use smudiff_core::side::Side;
use smudiff_view::divider::DIVIDER_WIDTH;
use smudiff_view::{
    FileDiff, FoldId, FoldSettings, Folding, NavTarget, Navigation, PositionKeeper,
    ScrollAnimation, ScrollState, StripeMarker, Viewport, stripe_markers,
};

use crate::geometry::{SideGeometry, gutter_layout};
use crate::side_editor::{SideCache, SideEditor, StripeRects};
use crate::tabs::display_width;
use crate::text::TextStyle;
use crate::theme::UiTheme;

actions!(
    smudiff,
    [
        NextChange,
        PreviousChange,
        ExpandAllFolds,
        CollapseAllFolds,
        ExpandFold,
        CollapseFold,
        NextContextRange,
    ]
);

/// Varighet på animert hopp til endring.
pub const NAV_ANIMATION_MS: f32 = 170.;

/// Linjer utenfor viewporten som beholdes shapet per side.
const CACHE_CAPACITY: usize = 2000;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

fn idx(side: Side) -> usize {
    side.select(0, 1)
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
    diff: Rc<FileDiff>,
    folding: Rc<Folding>,
    theme: UiTheme,
    family: &'static str,
    style: Option<TextStyle>,
    scroll: ScrollState,
    master: Side,
    nav: Navigation,
    animation: Option<Animation>,
    initialized: bool,
    viewport_height: f32,
    markers: Rc<[Vec<StripeMarker>; 2]>,
    /// Lengste linje i tegn, regnet én gang. Gir maks horisontal scroll uten å shape alt.
    max_cols: usize,
    shared: Rc<Shared>,
    focus: FocusHandle,
}

impl FileDiffView {
    pub fn new(
        diff: FileDiff,
        theme: UiTheme,
        family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let max_cols = SIDES
            .iter()
            .flat_map(|&s| (0..diff.line_count(s)).map(move |l| (s, l)))
            .map(|(s, l)| display_width(diff.line(s, l)))
            .max()
            .unwrap_or(0);
        let folding = Folding::new(&diff, FoldSettings::default());
        Self {
            folding: Rc::new(folding),
            diff: Rc::new(diff),
            theme,
            family,
            style: None,
            scroll: ScrollState::default(),
            master: Side::Left,
            nav: Navigation::default(),
            animation: None,
            initialized: false,
            viewport_height: 0.,
            markers: Rc::new([Vec::new(), Vec::new()]),
            max_cols,
            shared: Rc::new(Shared {
                bounds: Cell::new(None),
                caches: RefCell::new([
                    SideCache::new(CACHE_CAPACITY),
                    SideCache::new(CACHE_CAPACITY),
                ]),
                stripes: RefCell::new([StripeRects::default(), StripeRects::default()]),
                widest: Cell::new(0.),
            }),
            focus: cx.focus_handle(),
        }
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
            .is_some_and(|s| s.font.family == self.family)
        {
            return;
        }
        let style = TextStyle::new(self.family, window);
        let mut caches = self.shared.caches.borrow_mut();
        for cache in caches.iter_mut() {
            cache.set_key(style.cache_key());
        }
        drop(caches);
        self.style = Some(style);
        self.update_markers();
    }

    /// Stripen følger visuelle linjer, så den må regnes på nytt når foldene endres.
    fn update_markers(&mut self) {
        let Some(style) = &self.style else {
            return;
        };
        let lh = style.line_height();
        self.markers = Rc::new(SIDES.map(|s| stripe_markers(&self.diff, &self.folding, s, lh)));
        *self.shared.stripes.borrow_mut() = Default::default();
    }

    /// `FoldingModelImpl.runBatchFoldingOperation`: animasjonen fullføres, og caret eller
    /// øverste linje holdes på plass. Umiddelbart, uten animasjon.
    fn change_folding(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&FileDiff, &mut Folding) -> bool,
    ) {
        let Some(vp) = self.viewport() else {
            return;
        };
        if let Some(a) = self.animation.take() {
            self.scroll = a.anim.target();
        }
        let keeper = PositionKeeper::save(&self.scroll, &self.folding, self.nav.caret_lines, vp);
        if !op(&self.diff, Rc::make_mut(&mut self.folding)) {
            return;
        }
        let carets = SIDES.map(|s| self.folding.caret_after(s, self.nav.caret_lines[idx(s)]));
        self.nav.caret_lines = carets;
        keeper.restore(&mut self.scroll, &self.diff, &self.folding, carets, vp);
        self.update_markers();
        cx.notify();
    }

    /// Side og logisk linje under musa, `None` i divideren.
    fn line_at(&self, position: Point<Pixels>) -> Option<(Side, usize)> {
        let (vp, bounds, style) = (
            self.viewport()?,
            self.shared.bounds.get()?,
            self.style.as_ref()?,
        );
        let geoms = side_geometry(&self.diff, style, bounds);
        let x = f32::from(position.x);
        let side = SIDES
            .into_iter()
            .find(|&s| geoms[idx(s)].bounds().contains(&x))?;
        let layout = self.folding.layout(side, vp.line_height);
        let y = f32::from(position.y - bounds.origin.y) + self.scroll.y(side);
        Some((
            side,
            layout.y_to_logical_line(y).min(layout.line_count() - 1),
        ))
    }

    fn fold_at(&self, position: Point<Pixels>) -> Option<FoldId> {
        let (side, line) = self.line_at(position)?;
        self.folding.fold_at_line(side, line)
    }

    /// `FoldingModelSupport.MyMouseListener`: hover gjelder blokken på begge sider.
    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.fold_at(ev.position);
        if self.folding.hovered() != id {
            Rc::make_mut(&mut self.folding).set_hovered(id);
            cx.notify();
        }
    }

    fn geometry(&self, bounds: Bounds<Pixels>) -> Option<[SideGeometry; 2]> {
        let style = self.style.as_ref()?;
        Some(side_geometry(&self.diff, style, bounds))
    }

    fn side_at(&self, x: Pixels) -> Side {
        let Some(bounds) = self.shared.bounds.get() else {
            return self.master;
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
        let Some([left, _]) = self.geometry(bounds) else {
            return 0.;
        };
        let content = (self.max_cols + 1) as f32 * style.char_width;
        content.max(self.shared.widest.get()) - (left.text.end - left.text.start)
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(vp) = self.viewport() else {
            return;
        };
        let delta = ev.delta.pixel_delta(px(vp.line_height));
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        self.animation = None;
        let side = self.side_at(ev.position.x);
        self.master = side;
        if dy != 0. {
            self.scroll
                .scroll_by(&self.diff, &self.folding, side, -dy, vp);
        }
        if dx != 0. {
            let max_x = self.max_scroll_x();
            self.scroll.scroll_x_to(self.scroll.x - dx, max_x);
        }
        cx.notify();
    }

    /// Klikk gir fokus og caret, som navigasjonen regner fra. Klikk på en fold åpner neste
    /// nivå på begge sider (`DiffLineSeparatorRenderer.doAction`).
    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let Some((side, line)) = self.line_at(ev.position) else {
            return;
        };
        self.nav.side = side;
        self.nav.caret_lines[idx(side)] = line;
        if let Some(id) = self.folding.fold_at_line(side, line) {
            self.change_folding(cx, |_, f| f.set_expanded(id, true));
            // Musa står fortsatt på linja, som nå kan være neste nivå
            let id = self.fold_at(ev.position);
            Rc::make_mut(&mut self.folding).set_hovered(id);
        }
        cx.notify();
    }

    fn expand_all(&mut self, _: &ExpandAllFolds, _: &mut Window, cx: &mut Context<Self>) {
        self.change_folding(cx, |_, f| f.expand_all(true));
    }

    fn collapse_all(&mut self, _: &CollapseAllFolds, _: &mut Window, cx: &mut Context<Self>) {
        self.change_folding(cx, |_, f| f.expand_all(false));
    }

    fn expand_fold(&mut self, _: &ExpandFold, _: &mut Window, cx: &mut Context<Self>) {
        let (side, line) = (self.nav.side, self.nav.caret_lines[idx(self.nav.side)]);
        self.change_folding(cx, |_, f| f.expand_at(side, line));
    }

    fn collapse_fold(&mut self, _: &CollapseFold, _: &mut Window, cx: &mut Context<Self>) {
        let (side, line) = (self.nav.side, self.nav.caret_lines[idx(self.nav.side)]);
        self.change_folding(cx, |_, f| f.collapse_at(side, line));
    }

    /// `TextDiffSettings.setContextRange`: foldene bygges på nytt med tilstanden bevart.
    fn next_context_range(&mut self, _: &NextContextRange, _: &mut Window, cx: &mut Context<Self>) {
        self.change_folding(cx, |diff, f| {
            *f = f.with_settings(diff, f.settings().next_context_range());
            true
        });
    }

    fn next_change(&mut self, _: &NextChange, _: &mut Window, cx: &mut Context<Self>) {
        let Some(vp) = self.viewport() else {
            return;
        };
        if let Some(target) = self.nav.go_next(&self.diff, &self.folding, vp) {
            self.animate_to(&target, cx);
        }
    }

    fn previous_change(&mut self, _: &PreviousChange, _: &mut Window, cx: &mut Context<Self>) {
        let Some(vp) = self.viewport() else {
            return;
        };
        if let Some(target) = self.nav.go_prev(&self.diff, &self.folding, vp) {
            self.animate_to(&target, cx);
        }
    }

    fn animate_to(&mut self, target: &NavTarget, cx: &mut Context<Self>) {
        // Midt i en animasjon står scroll der forrige frame satte den
        let from = self.scroll;
        self.master = self.nav.side;
        self.animation = Some(Animation {
            anim: ScrollAnimation::new(self.nav.side, from, target, NAV_ANIMATION_MS),
            start: Instant::now(),
        });
        cx.notify();
    }

    fn step(&mut self, window: &mut Window) {
        let Some(vp) = self.viewport() else {
            return;
        };
        if !self.initialized {
            self.initialized = true;
            // `InitialScrollPositionSupport` faller tilbake til første endring
            if !self.diff.blocks().is_empty() {
                let t = self.nav.go_to(&self.diff, &self.folding, 0, vp);
                self.scroll.y = t.scroll_y;
                self.scroll.x = t.scroll_x;
            }
        }
        if let Some(a) = &self.animation {
            let elapsed = a.start.elapsed().as_secs_f32() * 1000.;
            let (state, done) = a.anim.sample(elapsed);
            self.scroll = state;
            if done {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
        }
    }

    /// Ny viewport-høyde: klem master og regn slave på nytt.
    fn resize(&mut self, height: f32) {
        if height == self.viewport_height {
            return;
        }
        self.viewport_height = height;
        let Some(vp) = self.viewport() else {
            return;
        };
        if let Some(a) = &mut self.animation {
            // Målet er regnet for den gamle høyden. Hopp dit og la sync rette opp.
            self.scroll = a.anim.target();
            self.animation = None;
        }
        let master = self.master;
        let layout = self.folding.layout(master, vp.line_height);
        self.scroll.y[idx(master)] = layout.clamp_scroll(self.scroll.y(master), vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, master, vp);
    }
}

pub fn side_geometry(
    diff: &FileDiff,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
) -> [SideGeometry; 2] {
    let x0 = f32::from(bounds.origin.x);
    let width = f32::from(bounds.size.width);
    let side_width = ((width - DIVIDER_WIDTH) / 2.).floor().max(0.);
    SIDES.map(|side| {
        // Siste linjenummer bestemmer bredden, som `getMaxLineNumberString`
        let digits = diff.line_count(side).to_string().len();
        let gutter = gutter_layout(digits as f32 * style.number_char_width);
        let x = match side {
            Side::Left => x0,
            Side::Right => x0 + side_width + DIVIDER_WIDTH,
        };
        let w = match side {
            Side::Left => side_width,
            Side::Right => width - side_width - DIVIDER_WIDTH,
        };
        SideGeometry::new(side, x, w, gutter)
    })
}

/// Snapper til hele device-piksler så kanter og tekst ikke blir uskarpe midt i en scroll.
fn snap(y: f32, scale: f32) -> f32 {
    (y * scale).round() / scale
}

impl Focusable for FileDiffView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileDiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_style(window);
        self.resize(f32::from(window.viewport_size().height));
        self.step(window);

        let scale = window.scale_factor();
        let scroll_y = self.scroll.y.map(|y| snap(y, scale));
        let scroll_x = snap(self.scroll.x, scale);
        let diff = self.diff.clone();
        let folding = self.folding.clone();
        let theme = self.theme;
        let style = self.style.clone();
        let markers = self.markers.clone();
        let shared = self.shared.clone();
        let carets = self.nav.caret_lines;

        div()
            .size_full()
            .bg(theme.editor_bg)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::next_change))
            .on_action(cx.listener(Self::previous_change))
            .on_action(cx.listener(Self::expand_all))
            .on_action(cx.listener(Self::collapse_all))
            .on_action(cx.listener(Self::expand_fold))
            .on_action(cx.listener(Self::collapse_fold))
            .on_action(cx.listener(Self::next_context_range))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(self.folding.hovered().is_some(), |d| d.cursor_pointer())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        let Some(style) = style else {
                            return;
                        };
                        shared.bounds.set(Some(bounds));
                        let top = f32::from(bounds.origin.y);
                        let height = f32::from(bounds.size.height);
                        let geoms = side_geometry(&diff, &style, bounds);
                        let mut caches = shared.caches.borrow_mut();
                        let mut stripes = shared.stripes.borrow_mut();
                        let mut widest = shared.widest.get();
                        for (i, geom) in geoms.iter().enumerate() {
                            let side = SIDES[i];
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
                                caret_line: carets[i],
                                stripe: &stripes[i],
                            };
                            widest = widest.max(editor.paint(&mut caches[i], window, cx));
                        }
                        shared.widest.set(widest);
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
                .size_full(),
            )
    }
}
