//! The size of the side panel, dragged with the mouse. The width follows the content until it is dragged
//! at the right edge. The height of a section at the bottom of the side panel is dragged at its top edge.

use gpui::{Context, Div, DragMoveEvent, prelude::*, px};

use crate::drag_handle::drag_handle;

const MIN_WIDTH: f32 = 320.;
/// Automatic width never takes more than this of the window.
const MAX_AUTO_FRACTION: f32 = 0.4;
const MAX_FRACTION: f32 = 0.8;
const MIN_HEIGHT: f32 = 80.;
const DEFAULT_HEIGHT_FRACTION: f32 = 1. / 3.;
const HANDLE_WIDTH: f32 = 20.;
/// The height handle lies above the edge, so it doesn't cover the first row.
const HANDLE_HEIGHT: f32 = 8.;

struct WidthDrag;
struct HeightDrag;

/// Unscaled, so the width follows the zoom.
#[derive(Default)]
pub struct SidePanelWidth {
    /// Set with the mouse. `None` follows the content.
    manual: Option<f32>,
}

impl SidePanelWidth {
    pub fn resolve(&self, content: f32, window: f32) -> f32 {
        match self.manual {
            Some(w) => w.min(window * MAX_FRACTION),
            None => content.min(window * MAX_AUTO_FRACTION),
        }
        .max(MIN_WIDTH)
    }

    /// The handle at the edge. A double click goes back to following the content. `get` gives the width
    /// and the zoom from the view's state.
    pub fn attach<T: 'static>(
        side_panel: Div,
        cx: &mut Context<T>,
        get: fn(&mut T) -> (&mut SidePanelWidth, f32),
    ) -> Div {
        side_panel
            .relative()
            .on_drag_move(
                cx.listener(move |this, e: &DragMoveEvent<WidthDrag>, _, cx| {
                    let (width, scale) = get(this);
                    width.manual = Some(f32::from(e.event.position.x - e.bounds.left()) / scale);
                    cx.notify();
                }),
            )
            .child(
                drag_handle(
                    "resize-width",
                    WidthDrag,
                    cx,
                    |_, _| {},
                    move |this, _| get(this).0.manual = None,
                )
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(HANDLE_WIDTH))
                .cursor_col_resize(),
            )
    }
}

/// Unscaled, like `SidePanelWidth`.
#[derive(Default)]
pub struct SectionHeight {
    /// Set with the mouse. `None` is a fixed share of the side panel.
    manual: Option<f32>,
}

impl SectionHeight {
    pub fn resolve(&self, window: f32) -> f32 {
        let max = (window * MAX_FRACTION).max(MIN_HEIGHT);
        self.manual
            .unwrap_or(window * DEFAULT_HEIGHT_FRACTION)
            .clamp(MIN_HEIGHT, max)
    }

    /// Puts `section` at the bottom of `side_panel`, with the handle at its top edge. A double click goes
    /// back to the default height.
    pub fn attach<T: 'static>(
        side_panel: Div,
        section: Div,
        cx: &mut Context<T>,
        get: fn(&mut T) -> (&mut SectionHeight, f32),
    ) -> Div {
        side_panel
            .on_drag_move(
                cx.listener(move |this, e: &DragMoveEvent<HeightDrag>, _, cx| {
                    let (height, scale) = get(this);
                    height.manual = Some(f32::from(e.bounds.bottom() - e.event.position.y) / scale);
                    cx.notify();
                }),
            )
            .child(
                section.flex_none().relative().child(
                    drag_handle(
                        "resize-height",
                        HeightDrag,
                        cx,
                        |_, _| {},
                        move |this, _| get(this).0.manual = None,
                    )
                    .left_0()
                    .right_0()
                    .top(px(-HANDLE_HEIGHT / 2.))
                    .h(px(HANDLE_HEIGHT))
                    .cursor_row_resize(),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_within_limits() {
        let auto = SidePanelWidth::default();
        assert_eq!(auto.resolve(350., 1000.), 350.);
        assert_eq!(auto.resolve(600., 1000.), 400.);
        assert_eq!(auto.resolve(50., 1000.), MIN_WIDTH);
        let manual = SidePanelWidth { manual: Some(700.) };
        assert_eq!(manual.resolve(300., 1000.), 700.);
        assert_eq!(manual.resolve(300., 500.), 400.);
    }

    #[test]
    fn height_within_limits() {
        assert_eq!(SectionHeight::default().resolve(900.), 300.);
        assert_eq!(SectionHeight::default().resolve(90.), MIN_HEIGHT);
        let manual = |h| SectionHeight { manual: Some(h) };
        assert_eq!(manual(500.).resolve(900.), 500.);
        assert_eq!(manual(10.).resolve(900.), MIN_HEIGHT);
        assert_eq!(manual(900.).resolve(900.), 720.);
    }
}
