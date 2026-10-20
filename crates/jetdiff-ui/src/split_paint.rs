//! Tegner split: to `SideEditor` med trakten mellom, eller én side alene for nye og
//! slettede filer.

use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Window};
use jetdiff_core::side::Side;
use jetdiff_view::{FileDiff, Folding, SideGeometry, StripeMarker, Viewport};

use crate::divider::Divider;
use crate::editor_paint::StripeRects;
use crate::side_editor::{CaretRow, SideCache, SideEditor};
use crate::text::TextStyle;
use crate::theme::UiTheme;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// Det som skal tegnes i én frame. Lages i `render` og tegnes i canvas-en.
pub struct SplitPainter {
    pub diff: Rc<FileDiff>,
    pub folding: Rc<Folding>,
    pub theme: UiTheme,
    pub style: TextStyle,
    pub markers: Rc<[Vec<StripeMarker>; 2]>,
    pub scroll_y: [f32; 2],
    pub scroll_x: f32,
    pub carets: [CaretRow; 2],
}

impl SplitPainter {
    /// Returnerer bredden av den bredeste synlige linja, for maks horisontal scroll.
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
        let single = self.diff.single_side();
        let mut widest: f32 = 0.;
        for (index, geom) in geometries.iter().enumerate() {
            let side = SIDES[index];
            if single.is_some_and(|shown| shown != side) {
                continue;
            }
            if stripes[index].track_height != height {
                let content_height = self.folding.layout(side, line_height).content_height();
                stripes[index] =
                    StripeRects::new(&self.markers[index], content_height, height, &self.theme);
            }
            let editor = SideEditor {
                diff: &self.diff,
                folding: &self.folding,
                geom,
                theme: &self.theme,
                style: &self.style,
                top,
                height,
                scroll_y: self.scroll_y[index],
                scroll_x: self.scroll_x,
                caret: self.carets[index],
                stripe: &stripes[index],
            };
            widest = widest.max(editor.paint(&mut caches[index], window, cx));
        }
        if single.is_none() {
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
        }
        widest
    }
}

pub fn side_geometry(
    diff: &FileDiff,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
) -> [SideGeometry; 2] {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    jetdiff_view::side_geometry(diff, style.number_char_width, x, width)
}
