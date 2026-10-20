//! Det UI-et gjør ulikt i split og unified: tegning, hva musa treffer og tekstbredden.
//! `FileDiffView` lager en `Painter` i `render` og tegner den i canvas-en.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Point, Window};
use jetdiff_core::side::Side;
use jetdiff_view::{DiffMode, ScrollState, StripeMarker, Viewport};

use crate::editor_paint::StripeRects;
use crate::side_editor::{CaretRow, SideCache};
use crate::split::{self, SplitPainter};
use crate::text::TextStyle;
use crate::theme::UiTheme;
use crate::unified_editor::{self, RowCache, UnifiedPainter};

/// Linjer utenfor viewporten som beholdes shapet per side.
const CACHE_CAPACITY: usize = 2000;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// Det paint deler med hendelsene: siste bounds, shapede linjer og error stripe.
pub struct Shared {
    pub bounds: Cell<Option<Bounds<Pixels>>>,
    /// Bredeste linje som er tegnet, for maks horisontal scroll.
    pub widest: Cell<f32>,
    split_lines: RefCell<[SideCache; 2]>,
    split_stripes: RefCell<[StripeRects; 2]>,
    unified_rows: RefCell<RowCache>,
    unified_stripe: RefCell<StripeRects>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            bounds: Cell::new(None),
            widest: Cell::new(0.),
            split_lines: RefCell::new(new_side_caches()),
            split_stripes: RefCell::default(),
            unified_rows: RefCell::new(RowCache::new(CACHE_CAPACITY)),
            unified_stripe: RefCell::default(),
        }
    }

    /// Ny fil eller nytt innhold.
    pub fn clear(&self) {
        *self.split_lines.borrow_mut() = new_side_caches();
        *self.unified_rows.borrow_mut() = RowCache::new(CACHE_CAPACITY);
        self.widest.set(0.);
    }

    /// Nøkkelen har fontstørrelsen, så zoom tømmer cachen.
    pub fn set_style(&self, style: &TextStyle) {
        for cache in self.split_lines.borrow_mut().iter_mut() {
            cache.set_key(style.cache_key());
        }
        self.unified_rows.borrow_mut().set_key(style.cache_key());
        self.widest.set(0.);
    }

    /// Stripen regnes ut på nytt neste gang den tegnes.
    pub fn clear_stripes(&self) {
        *self.split_stripes.borrow_mut() = Default::default();
        *self.unified_stripe.borrow_mut() = Default::default();
    }
}

fn new_side_caches() -> [SideCache; 2] {
    [
        SideCache::new(CACHE_CAPACITY),
        SideCache::new(CACHE_CAPACITY),
    ]
}

/// Det som er under musa.
pub enum Hit {
    Split(Side, usize),
    Unified(usize),
}

pub fn hit(
    mode: &DiffMode,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
    scroll_y: [f32; 2],
    position: Point<Pixels>,
) -> Option<Hit> {
    match mode {
        DiffMode::Split(state) => split::hit(
            &state.diff,
            &state.folding,
            style,
            bounds,
            scroll_y,
            position,
        )
        .map(|(side, line)| Hit::Split(side, line)),
        DiffMode::Unified(state) => {
            unified_editor::hit(&state.diff, style, bounds, scroll_y[0], position).map(Hit::Unified)
        }
    }
}

/// Split scroller siden musa står over, og divideren regnes til nærmeste side.
pub fn wheel(
    mode: &mut DiffMode,
    bounds: Option<Bounds<Pixels>>,
    x: Pixels,
    delta_y: f32,
    viewport: Viewport,
) {
    match mode {
        DiffMode::Split(state) => {
            let side = bounds.map_or(state.master, |bounds| split::side_at(bounds, x));
            state.wheel(side, delta_y, viewport);
        }
        DiffMode::Unified(state) => {
            state.wheel(delta_y, viewport);
        }
    }
}

pub fn text_width(mode: &DiffMode, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    match mode {
        DiffMode::Split(state) => split::text_width(&state.diff, style, bounds),
        DiffMode::Unified(state) => unified_editor::text_width(&state.diff, style, bounds),
    }
}

/// Det som skal tegnes i én frame.
pub enum Painter {
    Split(SplitPainter),
    Unified(UnifiedPainter),
}

impl Painter {
    /// `shown` er scroll slik den vises nå, midt i en eventuell animasjon.
    pub fn new(
        mode: &DiffMode,
        markers: &Rc<[Vec<StripeMarker>; 2]>,
        style: TextStyle,
        shown: ScrollState,
        focused: bool,
        scale: f32,
        theme: UiTheme,
    ) -> Self {
        let scroll_x = snap(shown.x, scale);
        match mode {
            DiffMode::Split(state) => {
                // Den andre siden viser overført linje svakere, så man ser hvor H og L lander.
                // IntelliJ har caret row bare på aktiv side.
                let cursor = state.cursor;
                let other = state.transferred_cursor();
                let carets = SIDES.map(|side| {
                    let caret = if side == cursor.side { cursor } else { other };
                    CaretRow {
                        line: caret.line,
                        active: focused && side == cursor.side,
                    }
                });
                Self::Split(SplitPainter {
                    diff: state.diff.clone(),
                    folding: state.folding.clone(),
                    theme,
                    style,
                    markers: markers.clone(),
                    scroll_y: shown.y.map(|y| snap(y, scale)),
                    scroll_x,
                    carets,
                })
            }
            DiffMode::Unified(state) => Self::Unified(UnifiedPainter {
                diff: state.diff.clone(),
                theme,
                style,
                scroll_y: snap(shown.y[0], scale),
                scroll_x,
                caret: CaretRow {
                    line: state.row,
                    active: focused,
                },
            }),
        }
    }

    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        shared: &Shared,
        window: &mut Window,
        cx: &mut App,
    ) {
        let old = shared.bounds.replace(Some(bounds));
        if old.map(|old| old.size.height) != Some(bounds.size.height) {
            window.refresh();
        }
        let widest = match self {
            Self::Split(painter) => painter.paint(
                bounds,
                &mut shared.split_lines.borrow_mut(),
                &mut shared.split_stripes.borrow_mut(),
                window,
                cx,
            ),
            Self::Unified(painter) => painter.paint(
                bounds,
                &mut shared.unified_rows.borrow_mut(),
                &mut shared.unified_stripe.borrow_mut(),
                window,
                cx,
            ),
        };
        shared.widest.set(shared.widest.get().max(widest));
    }
}

/// Snapper til hele device-piksler så kanter og tekst ikke blir uskarpe midt i en scroll.
fn snap(y: f32, scale: f32) -> f32 {
    (y * scale).round() / scale
}
