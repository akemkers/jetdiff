//! Én editor: bakgrunner, ord-markering, tekst, linjenumre og error stripe. Tegner bare
//! det `jetdiff-view` gir, og bare synlige linjer.

use gpui::{App, ShapedLine, SharedString, Window, fill, point, px};
use jetdiff_view::separator::{stroke_width, wave};
use jetdiff_view::{FileDiff, Folding, SideGeometry};

use crate::cache::LineCache;
use crate::divider::stroke_path;
use crate::editor_paint::{ShapedText, StripeRects, mask, rect, shape_number};
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub struct CachedLine {
    pub text: ShapedText,
    pub number: ShapedLine,
}

pub type SideCache = LineCache<(SharedString, u32), CachedLine>;

pub struct SideEditor<'a> {
    pub diff: &'a FileDiff,
    pub folding: &'a Folding,
    pub geom: &'a SideGeometry,
    pub theme: &'a UiTheme,
    pub style: &'a TextStyle,
    pub top: f32,
    pub height: f32,
    pub scroll_y: f32,
    pub scroll_x: f32,
    pub caret: CaretRow,
    pub stripe: &'a StripeRects,
}

/// Den markerte linja slik denne siden tegner den.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaretRow {
    pub line: usize,
    /// Aktiv side med fokus får full farge, så man ser hvor `o` virker.
    pub active: bool,
}

impl SideEditor<'_> {
    /// Returnerer bredden av den bredeste synlige linja, for maks horisontal scroll.
    pub fn paint(&self, cache: &mut SideCache, window: &mut Window, cx: &mut App) -> f32 {
        let g = self.geom;
        let side = g.side;
        let lh = self.style.line_height();
        let layout = self.folding.layout(side, lh);
        let (top, bottom) = (self.top, self.top + self.height);
        let screen_y = |y: f32| self.top + y - self.scroll_y;

        let side_bounds = rect(g.bounds(), top, bottom);
        let text_bounds = rect(g.text_x..g.text.end, top, bottom);
        let gutter_bounds = rect(g.gutter.clone(), top, bottom);
        let gl = g.gutter_layout;

        let (first, last) = layout.painted_lines(self.scroll_y, self.height);
        let last_line = last.min(layout.line_count() - 1);
        let (first_visual, last_visual) = layout.painted_visual_lines(self.scroll_y, self.height);
        let last_visual = last_visual.min(layout.visual_line_count() - 1);
        // Kollapsede folds har ingen tekst og ikke linjenummer (`hideLineNumberPredicate`)
        let lines: Vec<usize> = (first_visual..=last_visual)
            .filter(|&v| layout.fold_at_visual(v).is_none())
            .map(|v| layout.visual_to_logical(v))
            .collect();
        let mut widest: f32 = 0.;

        window.with_content_mask(mask(side_bounds), |window| {
            window.paint_quad(fill(side_bounds, self.theme.editor_bg));
            window.paint_quad(fill(gutter_bounds, self.theme.editor_bg));
            // New UI: 1px strek ved separatoren, males før diff-markørene så den skjules i endringer
            window.paint_quad(fill(
                rect(g.gutter_x(gl.separator..gl.separator + 1.), top, bottom),
                self.theme.indent_guide,
            ));

            self.paint_block_backgrounds(first, last, window);
            // Over diff-bakgrunnene, ellers forsvinner den i endringer
            if (first..=last_line).contains(&self.caret.line) {
                let y = screen_y(layout.line_top(self.caret.line));
                let color = if self.caret.active {
                    self.theme.row_highlight
                } else {
                    self.theme.row_highlight.opacity(0.4)
                };
                window.paint_quad(fill(rect(g.text.clone(), y, y + lh), color));
                // Linjenummeret er lettest å finne igjen etter et hopp
                if self.caret.active {
                    let numbers = g.gutter_x(0.0..gl.separator);
                    window.paint_quad(fill(rect(numbers, y, y + lh), self.theme.gutter_highlight));
                }
            }
            self.paint_folds(first_visual..=last_visual, window);

            window.with_content_mask(mask(text_bounds), |window| {
                let text_x = g.text_x - self.scroll_x;
                for &line in &lines {
                    let y = screen_y(layout.line_top(line));
                    let entry = cache.get_or_insert_with(line, || self.shape(line, window));
                    widest = widest.max(entry.text.width());
                    entry.text.paint_inline(
                        self.diff.line_highlight(side, line).inline,
                        text_x,
                        g.text.end,
                        y..y + lh,
                        self.theme,
                        window,
                    );
                    if let Some(shaped) = &entry.text.shaped {
                        let origin = point(px(text_x), px(y + self.style.text_dy));
                        let _ = shaped.paint(origin, px(lh), window, cx);
                    }
                }
            });

            window.with_content_mask(mask(gutter_bounds), |window| {
                for &line in &lines {
                    let y = screen_y(layout.line_top(line)) + self.style.number_dy;
                    let caret;
                    let number = if line == self.caret.line {
                        caret = shape_number(
                            line,
                            self.theme.line_number_caret_row,
                            self.style,
                            window,
                        );
                        &caret
                    } else {
                        &cache
                            .get_or_insert_with(line, || self.shape(line, window))
                            .number
                    };
                    let x = g.number_x(f32::from(number.width));
                    let _ = number.paint(point(px(x), px(y)), px(lh), window, cx);
                }
            });

            self.stripe.paint(
                g,
                self.top,
                self.scroll_y,
                layout.content_height(),
                self.theme,
                window,
            );
        });

        cache.trim(first..last_line + 1);
        widest
    }

    /// `DiffLineSeparatorRenderer`: bølge over gutter og tekst, med fasen fra divideren.
    fn paint_folds(&self, visual: std::ops::RangeInclusive<usize>, window: &mut Window) {
        let g = self.geom;
        let lh = self.style.line_height();
        let layout = self.folding.layout(g.side, lh);
        let edge = if g.mirrored {
            g.gutter.end
        } else {
            g.gutter.start
        };
        let hovered = self.folding.hovered();
        for (id, fold) in self.folding.visible(g.side) {
            if fold.visual < *visual.start() {
                continue;
            }
            if fold.visual > *visual.end() {
                break;
            }
            let y = self.top + layout.visual_line_to_y(fold.visual) - self.scroll_y;
            let path = wave(g.bounds(), edge, g.mirrored, y, lh);
            let width = stroke_width(hovered == Some(id));
            let p = |pt: jetdiff_view::Point| point(px(pt.x), px(pt.y));
            stroke_path(window, &path, p, width, self.theme.separator_wave);
        }
    }

    /// Per blokk, ikke per linje, så en stor endring blir ett rektangel.
    fn paint_block_backgrounds(&self, first: usize, last: usize, window: &mut Window) {
        let g = self.geom;
        let side = g.side;
        let gl = g.gutter_layout;
        let layout = self.folding.layout(side, self.style.line_height());
        let blocks = self.diff.blocks();
        let start = blocks.partition_point(|b| b.lines(side).end < first);
        let whole = g.text.start.min(g.gutter.start)..g.text.end.max(g.gutter.end);
        let outer = {
            // Ytterste 3px av gutteren og hele tekstområdet
            let sep = g.gutter_x(gl.separator..gl.width);
            sep.start.min(g.text.start)..sep.end.max(g.text.end)
        };

        for block in &blocks[start..] {
            let lines = block.lines(side);
            if lines.start > last + 1 {
                break;
            }
            let colors = self.theme.diff(block.diff_type);
            let (y1, y2) = layout.block_band(lines.clone());
            let (y1, y2) = (self.top + y1 - self.scroll_y, self.top + y2 - self.scroll_y);
            if lines.is_empty() {
                window.paint_quad(fill(rect(whole.clone(), y1, y2), colors.background));
                continue;
            }
            let text_color = if block.ignored() {
                colors.ignored
            } else {
                colors.background
            };
            window.paint_quad(fill(
                rect(g.gutter_x(0.0..gl.separator), y1, y2),
                colors.background,
            ));
            window.paint_quad(fill(rect(outer.clone(), y1, y2), text_color));
        }
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
        // Standardfargen forsvinner mot diff-bakgrunnen, spesielt blå
        let changed = self.diff.line_highlight(side, line).block.is_some();
        let color = if changed {
            self.theme.line_number_caret_row
        } else {
            self.theme.line_number
        };
        CachedLine {
            text,
            number: shape_number(line, color, self.style, window),
        }
    }
}
