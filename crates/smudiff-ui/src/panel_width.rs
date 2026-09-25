//! Bredden på et panel. Følger innholdet, men kan dras i kanten til høyre.

use gpui::{ClickEvent, Context, Div, DragMoveEvent, EmptyView, div, prelude::*, px};

const MIN_WIDTH: f32 = 160.;
/// Automatisk bredde tar aldri mer enn dette av vinduet.
const MAX_AUTO_FRACTION: f32 = 0.4;
const MAX_FRACTION: f32 = 0.8;
const HANDLE_WIDTH: f32 = 20.;

struct Drag;

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
            .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<Drag>, _, cx| {
                let (width, scale) = get(this);
                width.manual = Some(f32::from(e.event.position.x - e.bounds.left()) / scale);
                cx.notify();
            }))
            .child(
                div()
                    .id("resize")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(HANDLE_WIDTH))
                    .cursor_col_resize()
                    .on_drag(Drag, |_, _, _, cx| cx.new(|_| EmptyView))
                    .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                        if e.click_count() == 2 {
                            get(this).0.manual = None;
                            cx.notify();
                        }
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn innenfor_grensene() {
        let auto = PanelWidth::default();
        assert_eq!(auto.resolve(300., 1000.), 300.);
        assert_eq!(auto.resolve(600., 1000.), 400.);
        assert_eq!(auto.resolve(50., 1000.), MIN_WIDTH);
        let manual = PanelWidth { manual: Some(700.) };
        assert_eq!(manual.resolve(300., 1000.), 700.);
        assert_eq!(manual.resolve(300., 500.), 400.);
    }
}
