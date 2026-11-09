//! Split: two `SplitSide` with the diff divider between them. Drawing, and where on screen the sides
//! and lines are. Added and deleted files are always shown Unified.

use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Point, Window};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::divider::DIVIDER_WIDTH;
use jetdiff_viewmodel::{
    FileDiff, Folding, HunkMinimapMark, SideGeometry, SplitShare, Viewport, diff_divider_offset,
    split_text_width,
};

use super::CodeLayout;
use super::code_paint::HunkMinimapRects;
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
    pub hunk_minimap_marks: Rc<[Vec<HunkMinimapMark>; 2]>,
    pub scroll_y: [f32; 2],
    pub scroll_x: f32,
    pub cursor_rows: [CursorRow; 2],
    pub split_share: SplitShare,
}

impl SplitPainter {
    /// Returns the width of the widest visible line, for max horizontal scroll.
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        caches: &mut [SideCache; 2],
        hunk_minimaps: &mut [HunkMinimapRects; 2],
        window: &mut Window,
        cx: &mut App,
    ) -> f32 {
        let top = f32::from(bounds.origin.y);
        let height = f32::from(bounds.size.height);
        let line_height = self.style.line_height();
        let code_layout = CodeLayout {
            style: &self.style,
            split_share: self.split_share,
        };
        let geometries = side_geometry(&self.diff, code_layout, bounds);
        let mut widest: f32 = 0.;
        for (index, geom) in geometries.iter().enumerate() {
            let side = SIDES[index];
            if hunk_minimaps[index].track_height != height {
                let content_height = self.folding.layout(side, line_height).content_height();
                hunk_minimaps[index] = HunkMinimapRects::new(
                    &self.hunk_minimap_marks[index],
                    content_height,
                    height,
                    &self.theme,
                );
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
                cursor_row: Some(self.cursor_rows[index]),
                hunk_minimap: Some(&hunk_minimaps[index]),
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
            hunks: 0..self.diff.hunks().len(),
        }
        .paint(window);
        widest
    }
}

pub(super) fn side_geometry(
    diff: &FileDiff,
    code_layout: CodeLayout,
    bounds: Bounds<Pixels>,
) -> [SideGeometry; 2] {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    let number_char_width = code_layout.style.number_char_width;
    jetdiff_viewmodel::side_geometry(diff, number_char_width, x, width, code_layout.split_share)
}

fn index(side: Side) -> usize {
    side.select(0, 1)
}

/// Side and logical line under the mouse, `None` in the divider.
pub fn hit(
    diff: &FileDiff,
    folding: &Folding,
    code_layout: CodeLayout,
    bounds: Bounds<Pixels>,
    scroll_y: [f32; 2],
    position: Point<Pixels>,
) -> Option<(Side, usize)> {
    let geometries = side_geometry(diff, code_layout, bounds);
    let x = f32::from(position.x);
    let side = SIDES
        .into_iter()
        .find(|&side| geometries[index(side)].bounds().contains(&x))?;
    let layout = folding.layout(side, code_layout.style.line_height());
    let y = f32::from(position.y - bounds.origin.y) + scroll_y[index(side)];
    let line = layout.y_to_logical_line(y).min(layout.line_count() - 1);
    Some((side, line))
}

/// The side the mouse wheel scrolls. The divider counts as the nearest side.
pub fn side_at(bounds: Bounds<Pixels>, split_share: SplitShare, x: Pixels) -> Side {
    let offset = diff_divider_offset(f32::from(bounds.size.width), split_share);
    if f32::from(x - bounds.origin.x) < offset + DIVIDER_WIDTH / 2. {
        Side::Left
    } else {
        Side::Right
    }
}

pub fn text_width(diff: &FileDiff, code_layout: CodeLayout, bounds: Bounds<Pixels>) -> f32 {
    let geometries = side_geometry(diff, code_layout, bounds);
    split_text_width(geometries.map(|geometry| geometry.text.end - geometry.text_x))
}
