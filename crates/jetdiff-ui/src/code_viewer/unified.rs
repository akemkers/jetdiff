//! The Unified view: one code view where deleted rows sit above the added ones, with old and new
//! line numbers in the gutter. Drawing, and which row the mouse hits.

use std::ops::{Range, RangeInclusive};
use std::rc::Rc;

use gpui::{App, Bounds, Hsla, Pixels, Point, ShapedLine, SharedString, Window, point, px};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::{InlineHighlight, UnifiedDiff, UnifiedGeometry};

use super::code_paint::{
    ShapedText, StripeRects, mask, paint_caret_row, paint_change, paint_code_background, rect,
    shape_number,
};
use super::split_side::CaretRow;
use crate::cache::LineCache;
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub struct CachedRow {
    pub text: ShapedText,
    /// Old and new number. Deleted rows have only old, added rows only new.
    pub numbers: [Option<ShapedLine>; 2],
}

pub type RowCache = LineCache<(SharedString, u32), CachedRow>;

/// What to draw in one frame. Made in `render` and drawn in the canvas.
pub struct UnifiedPainter {
    pub diff: Rc<UnifiedDiff>,
    pub theme: UiTheme,
    pub style: TextStyle,
    pub scroll_y: f32,
    pub scroll_x: f32,
    /// `line` is the row.
    pub caret: CaretRow,
}

/// The geometry and the rows that are visible in this frame.
struct Frame<'a> {
    painter: &'a UnifiedPainter,
    geom: UnifiedGeometry,
    top: f32,
    bottom: f32,
    visible: RangeInclusive<usize>,
}

impl UnifiedPainter {
    /// Paints the layers from the bottom up. Returns the width of the widest visible row, for
    /// max horizontal scroll.
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        cache: &mut RowCache,
        stripe: &mut StripeRects,
        window: &mut Window,
        cx: &mut App,
    ) -> f32 {
        let top = f32::from(bounds.origin.y);
        let height = f32::from(bounds.size.height);
        let layout = self.diff.layout(self.style.line_height());
        let content_height = layout.content_height();
        if stripe.track_height != height {
            let markers = self.diff.stripe_markers(self.style.line_height());
            *stripe = StripeRects::new(&markers, content_height, height, &self.theme);
        }
        let (first, last) = layout.painted_lines(self.scroll_y, height);
        let frame = Frame {
            painter: self,
            geom: geometry(&self.diff, &self.style, bounds),
            top,
            bottom: top + height,
            visible: first..=last.min(self.diff.row_count() - 1),
        };
        let code = &frame.geom.code;

        let widest = window.with_content_mask(mask(frame.area(code.bounds())), |window| {
            paint_code_background(code, frame.top, frame.bottom, &self.theme, window);
            frame.paint_changes(window);
            frame.paint_caret_row(window);
            let widest = frame.paint_text(cache, window, cx);
            frame.paint_line_numbers(cache, window, cx);
            stripe.paint(
                code,
                top,
                self.scroll_y,
                content_height,
                &self.theme,
                window,
            );
            widest
        });
        cache.trim(first..last + 1);
        widest
    }

    fn shape(&self, row_index: usize, window: &mut Window) -> CachedRow {
        let row = self.diff.row(row_index);
        let text = ShapedText::new(
            self.diff.text(row_index),
            self.diff.diff.line_syntax(row.side(), row.line()),
            &self.style,
            &self.theme,
            window,
        );
        // The default color disappears against the diff background
        let color = if row.block.is_some() {
            self.theme.line_number_caret_row
        } else {
            self.theme.line_number
        };
        CachedRow {
            text,
            numbers: self.shape_numbers(row_index, color, window),
        }
    }

    fn shape_numbers(
        &self,
        row_index: usize,
        color: Hsla,
        window: &mut Window,
    ) -> [Option<ShapedLine>; 2] {
        self.diff
            .row(row_index)
            .lines
            .map(|line| line.map(|line| shape_number(line, color, &self.style, window)))
    }
}

impl Frame<'_> {
    /// The whole height of the code view, from `x.start` to `x.end`.
    fn area(&self, x: Range<f32>) -> Bounds<Pixels> {
        rect(x, self.top, self.bottom)
    }

    /// Y on screen for a y in the document.
    fn screen_y(&self, y: f32) -> f32 {
        self.top + y - self.painter.scroll_y
    }

    fn row_top(&self, row: usize) -> f32 {
        self.screen_y(row as f32 * self.painter.style.line_height())
    }

    /// The deleted and added part of each hunk get a color each, without blue.
    fn paint_changes(&self, window: &mut Window) {
        let diff = &self.painter.diff;
        let layout = diff.layout(self.painter.style.line_height());
        let blocks = diff.diff.blocks();
        for (index, rows) in diff.block_rows().iter().enumerate() {
            if rows.end <= *self.visible.start() {
                continue;
            }
            if rows.start > *self.visible.end() {
                break;
            }
            let deleted = diff.rows()[rows.clone()]
                .iter()
                .take_while(|row| row.side() == Side::Left)
                .count();
            let first_inserted = rows.start + deleted;
            for part in [rows.start..first_inserted, first_inserted..rows.end] {
                if part.is_empty() {
                    continue;
                }
                let colors = self.painter.theme.diff(diff.row(part.start).diff_type());
                let (band_top, band_bottom) = layout.block_band(part);
                let (top, bottom) = (self.screen_y(band_top), self.screen_y(band_bottom));
                let ignored = blocks[index].ignored();
                paint_change(&self.geom.code, top, bottom, colors, ignored, window);
            }
        }
    }

    fn paint_caret_row(&self, window: &mut Window) {
        let caret = self.painter.caret;
        if !self.visible.contains(&caret.line) {
            return;
        }
        let top = self.row_top(caret.line);
        let bottom = top + self.painter.style.line_height();
        let theme = &self.painter.theme;
        paint_caret_row(&self.geom.code, top, bottom, caret.active, theme, window);
    }

    /// Inline highlights and text, clipped so the text doesn't flow into the gutter on sideways scroll.
    /// Returns the width of the widest row.
    fn paint_text(&self, cache: &mut RowCache, window: &mut Window, cx: &mut App) -> f32 {
        let painter = self.painter;
        let code = &self.geom.code;
        let line_height = painter.style.line_height();
        let text_x = code.text_x - painter.scroll_x;
        let mut widest: f32 = 0.;
        window.with_content_mask(mask(self.area(code.text_x..code.text.end)), |window| {
            for row_index in self.visible.clone() {
                let row = painter.diff.row(row_index);
                let top = self.row_top(row_index);
                let entry =
                    cache.get_or_insert_with(row_index, || painter.shape(row_index, window));
                widest = widest.max(entry.text.width());
                // The color follows the row, not the word, so Unified doesn't get blue
                let highlights: Vec<InlineHighlight> = painter
                    .diff
                    .diff
                    .line_highlight(row.side(), row.line())
                    .inline
                    .iter()
                    .map(|highlight| InlineHighlight {
                        diff_type: row.diff_type(),
                        ..highlight.clone()
                    })
                    .collect();
                entry.text.paint_inline(
                    &highlights,
                    text_x,
                    code.text.end,
                    top..top + line_height,
                    &painter.theme,
                    window,
                );
                if let Some(shaped) = &entry.text.shaped {
                    let origin = point(px(text_x), px(top + painter.style.text_dy));
                    let _ = shaped.paint(origin, px(line_height), window, cx);
                }
            }
        });
        widest
    }

    fn paint_line_numbers(&self, cache: &mut RowCache, window: &mut Window, cx: &mut App) {
        let painter = self.painter;
        let line_height = painter.style.line_height();
        window.with_content_mask(mask(self.area(self.geom.code.gutter.clone())), |window| {
            for row_index in self.visible.clone() {
                let y = self.row_top(row_index) + painter.style.number_dy;
                let caret_numbers;
                let numbers = if row_index == painter.caret.line {
                    let color = painter.theme.line_number_caret_row;
                    caret_numbers = painter.shape_numbers(row_index, color, window);
                    &caret_numbers
                } else {
                    &cache
                        .get_or_insert_with(row_index, || painter.shape(row_index, window))
                        .numbers
                };
                for (number, side) in numbers.iter().zip([Side::Left, Side::Right]) {
                    let Some(number) = number else {
                        continue;
                    };
                    let width = f32::from(number.width);
                    let x = match side {
                        Side::Left => self.geom.old_number_x(width),
                        Side::Right => self.geom.new_number_x(width),
                    };
                    let _ = number.paint(point(px(x), px(y)), px(line_height), window, cx);
                }
            }
        });
    }
}

fn geometry(diff: &UnifiedDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> UnifiedGeometry {
    let (x, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
    UnifiedGeometry::new(&diff.diff, style.number_char_width, x, width)
}

/// The row under the mouse, `None` outside the code view.
pub fn hit(
    diff: &UnifiedDiff,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
    scroll_y: f32,
    position: Point<Pixels>,
) -> Option<usize> {
    let code = geometry(diff, style, bounds).code;
    if !code.bounds().contains(&f32::from(position.x)) {
        return None;
    }
    let layout = diff.layout(style.line_height());
    let y = f32::from(position.y - bounds.origin.y) + scroll_y;
    Some(layout.y_to_logical_line(y).min(layout.line_count() - 1))
}

/// The width the text has, for horizontal scroll.
pub fn text_width(diff: &UnifiedDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    let code = geometry(diff, style, bounds).code;
    code.text.end - code.text_x
}
