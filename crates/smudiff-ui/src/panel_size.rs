//! Størrelsen på paneler som kan dras. Bredden følger innholdet til den dras i kanten til
//! høyre. Høyden på en seksjon nederst i et panel dras i overkanten.

use gpui::{ClickEvent, Context, Div, DragMoveEvent, EmptyView, Stateful, div, prelude::*, px};

const MIN_WIDTH: f32 = 160.;
/// Automatisk bredde tar aldri mer enn dette av vinduet.
const MAX_AUTO_FRACTION: f32 = 0.4;
const MAX_FRACTION: f32 = 0.8;
const MIN_HEIGHT: f32 = 80.;
const DEFAULT_HEIGHT_FRACTION: f32 = 1. / 3.;
const HANDLE_WIDTH: f32 = 20.;
/// Høydehåndtaket ligger over kanten, så det ikke dekker første rad.
const HANDLE_HEIGHT: f32 = 8.;

/// Egne typer, siden `on_drag_move` lytter på alle drag av samme type.
struct WidthDrag;
struct HeightDrag;

/// Uskalert, så bredden følger zoomen.
#[derive(Default)]
pub struct PanelWidth {
    /// Satt med musa. `None` følger innholdet.
    manual: Option<f32>,
}

impl PanelWidth {
    pub fn resolve(&self, content: f32, window: f32) -> f32 {
        match self.manual {
            Some(w) => w.min(window * MAX_FRACTION),
            None => content.min(window * MAX_AUTO_FRACTION),
        }
        .max(MIN_WIDTH)
    }

    /// Håndtaket i kanten. Dobbeltklikk går tilbake til å følge innholdet. `get` gir bredden
    /// og zoomen fra panelets tilstand.
    pub fn attach<T: 'static>(
        panel: Div,
        cx: &mut Context<T>,
        get: fn(&mut T) -> (&mut PanelWidth, f32),
    ) -> Div {
        panel
            .relative()
            .on_drag_move(
                cx.listener(move |this, e: &DragMoveEvent<WidthDrag>, _, cx| {
                    let (width, scale) = get(this);
                    width.manual = Some(f32::from(e.event.position.x - e.bounds.left()) / scale);
                    cx.notify();
                }),
            )
            .child(
                handle("resize-width", WidthDrag, cx, move |this| {
                    get(this).0.manual = None
                })
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(HANDLE_WIDTH))
                .cursor_col_resize(),
            )
    }
}

/// Uskalert, som `PanelWidth`.
#[derive(Default)]
pub struct PanelHeight {
    /// Satt med musa. `None` er en fast andel av panelet.
    manual: Option<f32>,
}

impl PanelHeight {
    pub fn resolve(&self, window: f32) -> f32 {
        let max = (window * MAX_FRACTION).max(MIN_HEIGHT);
        self.manual
            .unwrap_or(window * DEFAULT_HEIGHT_FRACTION)
            .clamp(MIN_HEIGHT, max)
    }

    /// Legger `section` nederst i `panel`, med håndtaket i overkanten. Dobbeltklikk går
    /// tilbake til standardhøyden.
    pub fn attach<T: 'static>(
        panel: Div,
        section: Div,
        cx: &mut Context<T>,
        get: fn(&mut T) -> (&mut PanelHeight, f32),
    ) -> Div {
        panel
            .on_drag_move(
                cx.listener(move |this, e: &DragMoveEvent<HeightDrag>, _, cx| {
                    let (height, scale) = get(this);
                    height.manual = Some(f32::from(e.bounds.bottom() - e.event.position.y) / scale);
                    cx.notify();
                }),
            )
            .child(
                section.flex_none().relative().child(
                    handle("resize-height", HeightDrag, cx, move |this| {
                        get(this).0.manual = None
                    })
                    .left_0()
                    .right_0()
                    .top(px(-HANDLE_HEIGHT / 2.))
                    .h(px(HANDLE_HEIGHT))
                    .cursor_row_resize(),
                ),
            )
    }
}

/// Et håndtak som drar `drag`. Dobbeltklikk kaller `reset`.
fn handle<T: 'static, D: 'static>(
    id: &'static str,
    drag: D,
    cx: &mut Context<T>,
    reset: impl Fn(&mut T) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .on_drag(drag, |_, _, _, cx| cx.new(|_| EmptyView))
        .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
            if e.click_count() == 2 {
                reset(this);
                cx.notify();
            }
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bredde_innenfor_grensene() {
        let auto = PanelWidth::default();
        assert_eq!(auto.resolve(300., 1000.), 300.);
        assert_eq!(auto.resolve(600., 1000.), 400.);
        assert_eq!(auto.resolve(50., 1000.), MIN_WIDTH);
        let manual = PanelWidth { manual: Some(700.) };
        assert_eq!(manual.resolve(300., 1000.), 700.);
        assert_eq!(manual.resolve(300., 500.), 400.);
    }

    #[test]
    fn hoyde_innenfor_grensene() {
        assert_eq!(PanelHeight::default().resolve(900.), 300.);
        assert_eq!(PanelHeight::default().resolve(90.), MIN_HEIGHT);
        let manual = |h| PanelHeight { manual: Some(h) };
        assert_eq!(manual(500.).resolve(900.), 500.);
        assert_eq!(manual(10.).resolve(900.), MIN_HEIGHT);
        assert_eq!(manual(900.).resolve(900.), 720.);
    }
}
