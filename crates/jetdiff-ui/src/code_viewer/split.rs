//! The Split view: two `SplitSide` with the diff divider between them. Drawing, and where on screen the sides
//! and lines are. Added and deleted files are always shown Unified.

use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Point, Window};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::{FileDiff, Folding, SideGeometry, StripeMarker, Viewport};

use super::code_paint::StripeRects;
use super::divider::Divider;
use super::split_side::{CursorRow, SideCache, SplitSide};
use crate::text::TextStyle;
use crate::theme::UiTheme;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// What to draw in one frame. Made in `render` and drawn in the canvas.
pub struct SplitPainter {
    pub diff: Rc<FileDiff>,
    pub folding: Rc<Folding>,
    pub theme: UiTheme,
    pub style: TextStyle,
    pub markers: Rc<[Vec<StripeMarker>; 2]>,
    pub scroll_y: [f32; 2],
    pub scroll_x: f32,
    pub cursor_rows: [CursorRow; 2],
}

impl SplitPainter {
    /// Returns the width of the widest visible line, for max horizontal scroll.
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        caches: &mut [SideCache; 2],
        stripes: &mut [StripeRects; 2],
        window: &mut Window,
        cx: &mut App,
    ) -> f32 {
        let top = f32::from(bounds.origin.y);
        let height = f32::from(bounds.size.height);
        let line_height = self.style.line_height();
        let geometries = side_geometry(&self.diff, &self.style, bounds);
        let mut widest: f32 = 0.;
        for (index, geom) in geometries.iter().enumerate() {
            let side = SIDES[index];
            if stripes[index].track_height != height {
                let content_height = self.folding.layout(side, line_height).content_height();
                stripes[index] =
                    StripeRects::new(&self.markers[index], content_height, height, &self.theme);
            }
            let split_side = SplitSide {
                diff: &self.diff,
                folding: &self.folding,
                geom,
                theme: &self.theme,
                style: &self.style,
                top,
                height,
                scroll_y: self.scroll_y[index],
                scroll_x: self.scroll_x,
                cursor_row: self.cursor_rows[index],
                stripe: &stripes[index],
            };
            widest = widest.max(split_side.paint(&mut caches[index], window, cx));
        }
        Divider {
            diff: &self.diff,
            folding: &self.folding,
            theme: &self.theme,
            x: geometries[0].gutter.end,
            top,
            viewport: Viewport {
                height,
                line_height,
            },
            scroll_y: self.scroll_y,
        }
        .paint(window);
        widest
    }
}

fn side_geometry(diff: &FileDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> [SideGeometry; 2] {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    jetdiff_viewmodel::side_geometry(diff, style.number_char_width, x, width)
}

fn index(side: Side) -> usize {
    side.select(0, 1)
}

/// Side and logical line under the mouse, `None` in the divider.
pub fn hit(
    diff: &FileDiff,
    folding: &Folding,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
    scroll_y: [f32; 2],
    position: Point<Pixels>,
) -> Option<(Side, usize)> {
    let geometries = side_geometry(diff, style, bounds);
    let x = f32::from(position.x);
    let side = SIDES
        .into_iter()
        .find(|&side| geometries[index(side)].bounds().contains(&x))?;
    let layout = folding.layout(side, style.line_height());
    let y = f32::from(position.y - bounds.origin.y) + scroll_y[index(side)];
    let line = layout.y_to_logical_line(y).min(layout.line_count() - 1);
    Some((side, line))
}

/// The side the mouse wheel scrolls. The divider counts as the nearest side.
pub fn side_at(bounds: Bounds<Pixels>, x: Pixels) -> Side {
    if x < bounds.center().x {
        Side::Left
    } else {
        Side::Right
    }
}

/// The width the text has on one side. The sides are equally wide, so Left is enough.
pub fn text_width(diff: &FileDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    let [geom, _] = &side_geometry(diff, style, bounds);
    geom.text.end - geom.text_x
}
