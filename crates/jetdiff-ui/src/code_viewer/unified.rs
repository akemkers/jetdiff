//! Unified: one code view where deleted rows sit above the added ones, with a line
//! number for each side in the gutter. Drawing, and which row the mouse hits.

use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Hsla, Pixels, Point, ShapedLine, SharedString, Window, point, px};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::{
    InlineHighlight, LineLayout, UnifiedDiff, UnifiedFolding, UnifiedGeometry,
};

use super::code_paint::{
    HunkMinimapRects, ShapedText, mask, paint_code_background, paint_cursor_row, paint_hunk, rect,
    shape_number,
};
use super::fold_waves::FoldWaves;
use super::split_side::CursorRow;
use super::visible::Visible;
use crate::cache::LineCache;
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub struct CachedRow {
    pub text: ShapedText,
    /// The number on each side. Deleted rows have only Left, added rows only Right.
    pub numbers: [Option<ShapedLine>; 2],
}

pub type RowCache = LineCache<(SharedString, u32), CachedRow>;

/// What to draw in one frame. Made in `render` and drawn in the canvas.
pub struct UnifiedPainter {
    pub diff: Rc<UnifiedDiff>,
    pub folding: Rc<UnifiedFolding>,
    pub theme: UiTheme,
    pub style: TextStyle,
    pub scroll_y: f32,
    pub scroll_x: f32,
    /// `line` is the row. `None` in an excerpt, which has no cursor.
    pub cursor_row: Option<CursorRow>,
    /// An excerpt is too short for one.
    pub hunk_minimap: bool,
}

/// The geometry and the rows that are visible in this frame.
struct Frame<'a> {
    painter: &'a UnifiedPainter,
    layout: LineLayout<'a>,
    geom: UnifiedGeometry,
    top: f32,
    bottom: f32,
    /// The lines are rows.
    visible: Visible,
}

impl UnifiedPainter {
    /// Paints the layers from the bottom up. Returns the width of the widest visible row, for
    /// max horizontal scroll.
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        cache: &mut RowCache,
        hunk_minimap: &mut HunkMinimapRects,
        window: &mut Window,
        cx: &mut App,
    ) -> f32 {
        let top = f32::from(bounds.origin.y);
        let height = f32::from(bounds.size.height);
        let line_height = self.style.line_height();
        let layout = self.folding.layout(line_height);
        let content_height = layout.content_height();
        if hunk_minimap.track_height != height {
            let marks = self.diff.hunk_minimap_marks(&self.folding, line_height);
            *hunk_minimap = HunkMinimapRects::new(&marks, content_height, height, &self.theme);
        }
        let frame = Frame {
            painter: self,
            layout,
            geom: geometry(&self.diff, &self.style, bounds),
            top,
            bottom: top + height,
            visible: Visible::new(&layout, self.scroll_y, height),
        };
        let code = &frame.geom.code;

        let widest = window.with_content_mask(mask(frame.area(code.bounds())), |window| {
            paint_code_background(code, frame.top, frame.bottom, &self.theme, window);
            frame.paint_hunks(window);
            frame.paint_cursor_row(window);
            frame.paint_folds(window);
            let widest = frame.paint_text(cache, window, cx);
            frame.paint_line_numbers(cache, window, cx);
            if self.hunk_minimap {
                hunk_minimap.paint(
                    code,
                    top,
                    self.scroll_y,
                    content_height,
                    &self.theme,
                    window,
                );
            }
            widest
        });
        let logical = &frame.visible.logical;
        cache.trim(*logical.start()..*logical.end() + 1);
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
        let color = if row.hunk.is_some() {
            self.theme.cursor_row_line_number
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
        self.screen_y(self.layout.line_top(row))
    }

    /// The deleted and added part of each hunk get a color each, without blue.
    fn paint_hunks(&self, window: &mut Window) {
        let diff = &self.painter.diff;
        let layout = &self.layout;
        let hunks = diff.diff.hunks();
        for (index, rows) in diff.hunk_rows().iter().enumerate() {
            if rows.end <= *self.visible.logical.start() {
                continue;
            }
            if rows.start > *self.visible.logical.end() {
                break;
            }
            let deleted = diff.rows()[rows.clone()]
                .iter()
                .take_while(|row| row.side() == Side::Left)
                .count();
            let first_added = rows.start + deleted;
            for part in [rows.start..first_added, first_added..rows.end] {
                if part.is_empty() {
                    continue;
                }
                let colors = self.painter.theme.diff(diff.row(part.start).kind());
                let (band_top, band_bottom) = layout.hunk_band(part);
                let (top, bottom) = (self.screen_y(band_top), self.screen_y(band_bottom));
                let compared_by_word = hunks[index].compared_by_word();
                paint_hunk(
                    &self.geom.code,
                    top,
                    bottom,
                    colors,
                    compared_by_word,
                    window,
                );
            }
        }
    }

    fn paint_cursor_row(&self, window: &mut Window) {
        let Some(cursor_row) = self.painter.cursor_row else {
            return;
        };
        if !self.visible.logical.contains(&cursor_row.line) {
            return;
        }
        let top = self.row_top(cursor_row.line);
        let bottom = top + self.painter.style.line_height();
        let theme = &self.painter.theme;
        paint_cursor_row(
            &self.geom.code,
            top,
            bottom,
            cursor_row.active,
            theme,
            window,
        );
    }

    /// As on each side in Split, without a fold link.
    fn paint_folds(&self, window: &mut Window) {
        let painter = self.painter;
        let code = &self.geom.code;
        let waves = FoldWaves {
            layout: &self.layout,
            x: code.bounds(),
            edge: code.gutter.start,
            mirrored: false,
            document_top: self.screen_y(0.),
            hovered: painter.folding.hovered(),
            color: painter.theme.fold_wave,
        };
        waves.paint(painter.folding.visible(), &self.visible, window);
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
            for &row_index in &self.visible.lines {
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
                        kind: row.kind(),
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
            for &row_index in &self.visible.lines {
                let y = self.row_top(row_index) + painter.style.number_dy;
                let cursor_numbers;
                let is_cursor_row = painter
                    .cursor_row
                    .is_some_and(|cursor_row| cursor_row.line == row_index);
                let numbers = if is_cursor_row {
                    let color = painter.theme.cursor_row_line_number;
                    cursor_numbers = painter.shape_numbers(row_index, color, window);
                    &cursor_numbers
                } else {
                    &cache
                        .get_or_insert_with(row_index, || painter.shape(row_index, window))
                        .numbers
                };
                for (number, side) in numbers.iter().zip([Side::Left, Side::Right]) {
                    let Some(number) = number else {
                        continue;
                    };
                    let x = self.geom.number_x(side, f32::from(number.width));
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
    folding: &UnifiedFolding,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
    scroll_y: f32,
    position: Point<Pixels>,
) -> Option<usize> {
    let code = geometry(diff, style, bounds).code;
    if !code.bounds().contains(&f32::from(position.x)) {
        return None;
    }
    let layout = folding.layout(style.line_height());
    let y = f32::from(position.y - bounds.origin.y) + scroll_y;
    Some(layout.y_to_logical_line(y).min(layout.line_count() - 1))
}

/// The width the text has, for horizontal scroll.
pub fn text_width(diff: &UnifiedDiff, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    let code = geometry(diff, style, bounds).code;
    code.text.end - code.text_x
}
