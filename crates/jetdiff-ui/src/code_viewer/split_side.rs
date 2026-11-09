//! One side in Split: backgrounds, inline highlights, text, line numbers and the hunk minimap. Draws only
//! what `jetdiff-viewmodel` gives, and only visible lines.

use std::ops::Range;

use gpui::{App, Bounds, Pixels, ShapedLine, SharedString, Window, fill, point, px};
use jetdiff_viewmodel::{FileDiff, Folding, LineLayout, SideGeometry};

use super::code_paint::{
    HunkMinimapRects, ShapedText, mask, paint_code_background, paint_cursor_row, paint_hunk, rect,
    shape_number,
};
use super::fold_waves::FoldWaves;
use super::visible::Visible;
use crate::cache::LineCache;
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub struct CachedLine {
    pub text: ShapedText,
    pub number: ShapedLine,
}

pub type SideCache = LineCache<(SharedString, u32), CachedLine>;

pub struct SplitSide<'a> {
    pub diff: &'a FileDiff,
    pub folding: &'a Folding,
    pub geom: &'a SideGeometry,
    pub theme: &'a UiTheme,
    pub style: &'a TextStyle,
    pub top: f32,
    pub height: f32,
    pub scroll_y: f32,
    pub scroll_x: f32,
    /// `None` in an excerpt, which has no cursor.
    pub cursor_row: Option<CursorRow>,
    /// An excerpt is too short for one.
    pub hunk_minimap: Option<&'a HunkMinimapRects>,
}

/// The cursor's line as this side draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorRow {
    pub line: usize,
    /// The active side with focus gets full color, so you can see where `o` acts.
    pub active: bool,
}

impl SplitSide<'_> {
    /// Paints the layers from the bottom up. Returns the width of the widest visible line, for
    /// max horizontal scroll.
    pub fn paint(&self, cache: &mut SideCache, window: &mut Window, cx: &mut App) -> f32 {
        let visible = self.visible();
        let widest = window.with_content_mask(mask(self.area(self.geom.bounds())), |window| {
            let bottom = self.top + self.height;
            paint_code_background(self.geom, self.top, bottom, self.theme, window);
            self.paint_hunk_backgrounds(&visible, window);
            // Over the diff backgrounds, otherwise it disappears in hunks
            self.paint_cursor_row(&visible, window);
            self.paint_folds(&visible, window);
            let widest = self.paint_text(&visible, cache, window, cx);
            self.paint_line_numbers(&visible, cache, window, cx);
            if let Some(hunk_minimap) = self.hunk_minimap {
                let content_height = self.layout().content_height();
                hunk_minimap.paint(
                    self.geom,
                    self.top,
                    self.scroll_y,
                    content_height,
                    self.theme,
                    window,
                );
            }
            widest
        });
        cache.trim(*visible.logical.start()..*visible.logical.end() + 1);
        widest
    }

    fn layout(&self) -> LineLayout<'_> {
        self.folding
            .layout(self.geom.side, self.style.line_height())
    }

    /// Y on screen for a y in the document.
    fn screen_y(&self, y: f32) -> f32 {
        self.top + y - self.scroll_y
    }

    /// The whole height of the side, from `x.start` to `x.end`.
    fn area(&self, x: Range<f32>) -> Bounds<Pixels> {
        rect(x, self.top, self.top + self.height)
    }

    fn visible(&self) -> Visible {
        Visible::new(&self.layout(), self.scroll_y, self.height)
    }

    /// Per hunk, not per line, so a large hunk becomes one rectangle.
    fn paint_hunk_backgrounds(&self, visible: &Visible, window: &mut Window) {
        let geom = self.geom;
        let side = geom.side;
        let layout = self.layout();
        let (first, last) = (*visible.logical.start(), *visible.logical.end());
        let hunks = self.diff.hunks();
        let start = hunks.partition_point(|hunk| hunk.lines(side).end < first);
        let whole = geom.text.start.min(geom.gutter.start)..geom.text.end.max(geom.gutter.end);

        for hunk in &hunks[start..] {
            let lines = hunk.lines(side);
            if lines.start > last + 1 {
                break;
            }
            let colors = self.theme.diff(hunk.kind);
            let (band_top, band_bottom) = layout.hunk_band(lines.clone());
            let (top, bottom) = (self.screen_y(band_top), self.screen_y(band_bottom));
            if lines.is_empty() {
                window.paint_quad(fill(rect(whole.clone(), top, bottom), colors.background));
                continue;
            }
            paint_hunk(geom, top, bottom, colors, hunk.compared_by_word(), window);
        }
    }

    fn paint_cursor_row(&self, visible: &Visible, window: &mut Window) {
        let Some(cursor_row) = self.cursor_row else {
            return;
        };
        let line = cursor_row.line;
        if !visible.logical.contains(&line) {
            return;
        }
        let top = self.screen_y(self.layout().line_top(line));
        let bottom = top + self.style.line_height();
        paint_cursor_row(
            self.geom,
            top,
            bottom,
            cursor_row.active,
            self.theme,
            window,
        );
    }

    /// With the phase from the divider.
    fn paint_folds(&self, visible: &Visible, window: &mut Window) {
        let geom = self.geom;
        let edge = if geom.mirrored {
            geom.gutter.end
        } else {
            geom.gutter.start
        };
        let waves = FoldWaves {
            layout: &self.layout(),
            x: geom.bounds(),
            edge,
            mirrored: geom.mirrored,
            document_top: self.screen_y(0.),
            hovered: self.folding.hovered(),
            color: self.theme.fold_wave,
        };
        waves.paint(self.folding.visible(geom.side), visible, window);
    }

    /// Inline highlights and text, clipped so the text doesn't flow into the gutter on sideways scroll.
    /// Returns the width of the widest line.
    fn paint_text(
        &self,
        visible: &Visible,
        cache: &mut SideCache,
        window: &mut Window,
        cx: &mut App,
    ) -> f32 {
        let geom = self.geom;
        let side = geom.side;
        let line_height = self.style.line_height();
        let layout = self.layout();
        let text_x = geom.text_x - self.scroll_x;
        let mut widest: f32 = 0.;
        window.with_content_mask(mask(self.area(geom.text_x..geom.text.end)), |window| {
            for &line in &visible.lines {
                let top = self.screen_y(layout.line_top(line));
                let entry = cache.get_or_insert_with(line, || self.shape(line, window));
                widest = widest.max(entry.text.width());
                entry.text.paint_inline(
                    self.diff.line_highlight(side, line).inline,
                    text_x,
                    geom.text.end,
                    top..top + line_height,
                    self.theme,
                    window,
                );
                if let Some(shaped) = &entry.text.shaped {
                    let origin = point(px(text_x), px(top + self.style.text_dy));
                    let _ = shaped.paint(origin, px(line_height), window, cx);
                }
            }
        });
        widest
    }

    fn paint_line_numbers(
        &self,
        visible: &Visible,
        cache: &mut SideCache,
        window: &mut Window,
        cx: &mut App,
    ) {
        let geom = self.geom;
        let line_height = self.style.line_height();
        let layout = self.layout();
        window.with_content_mask(mask(self.area(geom.gutter.clone())), |window| {
            for &line in &visible.lines {
                let y = self.screen_y(layout.line_top(line)) + self.style.number_dy;
                let cursor_number;
                let number = if self
                    .cursor_row
                    .is_some_and(|cursor_row| cursor_row.line == line)
                {
                    let color = self.theme.cursor_row_line_number;
                    cursor_number = shape_number(line, color, self.style, window);
                    &cursor_number
                } else {
                    &cache
                        .get_or_insert_with(line, || self.shape(line, window))
                        .number
                };
                let x = geom.number_x(f32::from(number.width));
                let _ = number.paint(point(px(x), px(y)), px(line_height), window, cx);
            }
        });
    }

    fn shape(&self, line: usize, window: &mut Window) -> CachedLine {
        let side = self.geom.side;
        let text = ShapedText::new(
            self.diff.line(side, line),
            self.diff.line_syntax(side, line),
            self.style,
            self.theme,
            window,
        );
        // The default color disappears against the diff background, especially blue
        let in_hunk = self.diff.line_highlight(side, line).hunk.is_some();
        let color = if in_hunk {
            self.theme.cursor_row_line_number
        } else {
            self.theme.line_number
        };
        CachedLine {
            text,
            number: shape_number(line, color, self.style, window),
        }
    }
}
