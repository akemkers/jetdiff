//! Split-visningen: to `SideEditor` med trakten mellom, eller én side alene for nye og
//! slettede filer. Tegning, og hvor på skjermen sidene og linjene er.

use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Point, Window};
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

fn side_geometry(diff: &FileDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> [SideGeometry; 2] {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    jetdiff_view::side_geometry(diff, style.number_char_width, x, width)
}

fn index(side: Side) -> usize {
    side.select(0, 1)
}

/// Side og logisk linje under musa, `None` i divideren.
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

/// Siden musehjulet scroller. Divideren regnes til nærmeste side.
pub fn side_at(diff: &FileDiff, bounds: Bounds<Pixels>, x: Pixels) -> Side {
    if let Some(side) = diff.single_side() {
        return side;
    }
    if x < bounds.center().x {
        Side::Left
    } else {
        Side::Right
    }
}

/// Bredden teksten har på én side. Sidene er like brede, så venstre side holder når begge
/// vises.
pub fn text_width(diff: &FileDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    let side = diff.single_side().unwrap_or(Side::Left);
    let geom = &side_geometry(diff, style, bounds)[index(side)];
    geom.text.end - geom.text_x
}
