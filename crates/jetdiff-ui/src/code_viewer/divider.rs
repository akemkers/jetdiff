//! The diff divider between the sides. The geometry comes ready from `divider_bands` and
//! `divider_separators`, local to the divider with the same y axis as the sides.

use gpui::{Bounds, ContentMask, Hsla, PathBuilder, Pixels, Window, fill, point, px, size};
use jetdiff_viewmodel::divider::DIVIDER_WIDTH;
use jetdiff_viewmodel::separator::stroke_width;
use jetdiff_viewmodel::{
    Cubic, FileDiff, Folding, PathSeg, Point, Viewport, divider_bands, divider_separators,
};

use crate::theme::UiTheme;

/// Strokes a path from `jetdiff-viewmodel`. `map` gives window coordinates.
pub fn stroke_path(
    window: &mut Window,
    segs: &[PathSeg],
    map: impl Fn(Point) -> gpui::Point<Pixels>,
    width: f32,
    color: Hsla,
) {
    let mut b = PathBuilder::stroke(px(width));
    for seg in segs {
        match *seg {
            PathSeg::Move(p) => b.move_to(map(p)),
            PathSeg::Quad { ctrl, to } => b.curve_to(map(to), map(ctrl)),
            PathSeg::Cubic { c1, c2, to } => b.cubic_bezier_to(map(to), map(c1), map(c2)),
        }
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

pub struct Divider<'a> {
    pub diff: &'a FileDiff,
    pub folding: &'a Folding,
    pub theme: &'a UiTheme,
    pub x: f32,
    pub top: f32,
    pub viewport: Viewport,
    pub scroll_y: [f32; 2],
}

impl Divider<'_> {
    pub fn paint(&self, window: &mut Window) {
        let bounds = Bounds::new(
            point(px(self.x), px(self.top)),
            size(px(DIVIDER_WIDTH), px(self.viewport.height)),
        );
        let p = |pt: Point| point(px(self.x + pt.x), px(self.top + pt.y));
        let curve = |b: &mut PathBuilder, c: &Cubic| b.cubic_bezier_to(p(c.p3), p(c.p1), p(c.p2));

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, self.theme.editor_bg));
            for band in divider_bands(
                self.diff,
                self.folding,
                self.scroll_y,
                self.viewport,
                DIVIDER_WIDTH,
            ) {
                let color = self.theme.diff(band.diff_type).background;

                let mut b = PathBuilder::fill();
                b.move_to(p(band.top.p0));
                curve(&mut b, &band.top);
                b.line_to(p(band.bottom.p3));
                curve(&mut b, &band.bottom.reversed());
                b.close();
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }

                // Steep bands never get thinner than 1px
                let mut b = PathBuilder::stroke(px(1.));
                b.move_to(p(band.mid.p0));
                curve(&mut b, &band.mid);
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
            let seps =
                divider_separators(self.folding, self.scroll_y, self.viewport, DIVIDER_WIDTH);
            for sep in seps {
                let width = stroke_width(sep.hovered);
                stroke_path(window, &sep.path, p, width, self.theme.separator_wave);
            }
        });
    }
}
