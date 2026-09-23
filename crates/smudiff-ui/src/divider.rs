//! Trakten mellom editorene. Geometrien kommer ferdig fra `divider_bands`, lokal for
//! divideren med samme y-akse som editorene.

use gpui::{Bounds, ContentMask, PathBuilder, Window, fill, point, px, size};
use smudiff_view::divider::DIVIDER_WIDTH;
use smudiff_view::{Cubic, FileDiff, Point, Viewport, divider_bands};

use crate::theme::UiTheme;

pub struct Divider<'a> {
    pub diff: &'a FileDiff,
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
            window.paint_quad(fill(bounds, self.theme.divider_bg));
            for band in divider_bands(self.diff, self.scroll_y, self.viewport, DIVIDER_WIDTH) {
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

                // Bratte bånd blir aldri tynnere enn 1px
                let mut b = PathBuilder::stroke(px(1.));
                b.move_to(p(band.mid.p0));
                curve(&mut b, &band.mid);
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
        });
    }
}
