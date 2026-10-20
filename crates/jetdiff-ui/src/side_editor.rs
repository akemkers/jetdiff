//! Én editor: bakgrunner, ord-markering, tekst, linjenumre og error stripe. Tegner bare
//! det `jetdiff-view` gir, og bare synlige linjer.

use std::ops::{Range, RangeInclusive};

use gpui::{App, Bounds, Pixels, ShapedLine, SharedString, Window, fill, point, px};
use jetdiff_view::separator::{stroke_width, wave};
use jetdiff_view::{FileDiff, Folding, LineLayout, SideGeometry};

use crate::cache::LineCache;
use crate::divider::stroke_path;
use crate::editor_paint::{
    ShapedText, StripeRects, mask, paint_caret_row, paint_change, paint_editor_background, rect,
    shape_number,
};
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

/// Det som er synlig i viewporten denne framen.
struct Visible {
    /// Logiske linjer, også de som er skjult i en kollapset fold.
    logical: RangeInclusive<usize>,
    /// Visuelle linjer, der en kollapset fold er én linje.
    visual: RangeInclusive<usize>,
    /// Linjene som har tekst og nummer. Kollapsede folds har ingen av delene
    /// (`hideLineNumberPredicate`).
    lines: Vec<usize>,
}

impl SideEditor<'_> {
    /// Maler lagene nedenfra og opp. Returnerer bredden av den bredeste synlige linja, for
    /// maks horisontal scroll.
    pub fn paint(&self, cache: &mut SideCache, window: &mut Window, cx: &mut App) -> f32 {
        let visible = self.visible();
        let widest = window.with_content_mask(mask(self.area(self.geom.bounds())), |window| {
            let bottom = self.top + self.height;
            paint_editor_background(self.geom, self.top, bottom, self.theme, window);
            self.paint_block_backgrounds(&visible, window);
            // Over diff-bakgrunnene, ellers forsvinner den i endringer
            self.paint_caret_row(&visible, window);
            self.paint_folds(&visible, window);
            let widest = self.paint_text(&visible, cache, window, cx);
            self.paint_line_numbers(&visible, cache, window, cx);
            let content_height = self.layout().content_height();
            self.stripe.paint(
                self.geom,
                self.top,
                self.scroll_y,
                content_height,
                self.theme,
                window,
            );
            widest
        });
        cache.trim(*visible.logical.start()..*visible.logical.end() + 1);
        widest
    }

    fn layout(&self) -> LineLayout<'_> {
        self.folding
            .layout(self.geom.side, self.style.line_height())
    }

    /// Y på skjermen for en y i dokumentet.
    fn screen_y(&self, y: f32) -> f32 {
        self.top + y - self.scroll_y
    }

    /// Hele høyden til editoren, fra `x.start` til `x.end`.
    fn area(&self, x: Range<f32>) -> Bounds<Pixels> {
        rect(x, self.top, self.top + self.height)
    }

    fn visible(&self) -> Visible {
        let layout = self.layout();
        let (first, last) = layout.painted_lines(self.scroll_y, self.height);
        let (first_visual, last_visual) = layout.painted_visual_lines(self.scroll_y, self.height);
        let visual = first_visual..=last_visual.min(layout.visual_line_count() - 1);
        let lines = visual
            .clone()
            .filter(|&visual_line| layout.fold_at_visual(visual_line).is_none())
            .map(|visual_line| layout.visual_to_logical(visual_line))
            .collect();
        Visible {
            logical: first..=last.min(layout.line_count() - 1),
            visual,
            lines,
        }
    }

    /// Per blokk, ikke per linje, så en stor endring blir ett rektangel.
    fn paint_block_backgrounds(&self, visible: &Visible, window: &mut Window) {
        let geom = self.geom;
        let side = geom.side;
        let layout = self.layout();
        let (first, last) = (*visible.logical.start(), *visible.logical.end());
        let blocks = self.diff.blocks();
        let start = blocks.partition_point(|block| block.lines(side).end < first);
        let whole = geom.text.start.min(geom.gutter.start)..geom.text.end.max(geom.gutter.end);

        for block in &blocks[start..] {
            let lines = block.lines(side);
            if lines.start > last + 1 {
                break;
            }
            let colors = self.theme.diff(block.diff_type);
            let (band_top, band_bottom) = layout.block_band(lines.clone());
            let (top, bottom) = (self.screen_y(band_top), self.screen_y(band_bottom));
            if lines.is_empty() {
                window.paint_quad(fill(rect(whole.clone(), top, bottom), colors.background));
                continue;
            }
            paint_change(geom, top, bottom, colors, block.ignored(), window);
        }
    }

    fn paint_caret_row(&self, visible: &Visible, window: &mut Window) {
        let line = self.caret.line;
        if !visible.logical.contains(&line) {
            return;
        }
        let top = self.screen_y(self.layout().line_top(line));
        let bottom = top + self.style.line_height();
        paint_caret_row(
            self.geom,
            top,
            bottom,
            self.caret.active,
            self.theme,
            window,
        );
    }

    /// `DiffLineSeparatorRenderer`: bølge over gutter og tekst, med fasen fra divideren.
    fn paint_folds(&self, visible: &Visible, window: &mut Window) {
        let geom = self.geom;
        let line_height = self.style.line_height();
        let layout = self.layout();
        let edge = if geom.mirrored {
            geom.gutter.end
        } else {
            geom.gutter.start
        };
        let hovered = self.folding.hovered();
        for (id, fold) in self.folding.visible(geom.side) {
            if fold.visual < *visible.visual.start() {
                continue;
            }
            if fold.visual > *visible.visual.end() {
                break;
            }
            let y = self.screen_y(layout.visual_line_to_y(fold.visual));
            let path = wave(geom.bounds(), edge, geom.mirrored, y, line_height);
            let width = stroke_width(hovered == Some(id));
            let to_pixels = |p: jetdiff_view::Point| point(px(p.x), px(p.y));
            stroke_path(window, &path, to_pixels, width, self.theme.separator_wave);
        }
    }

    /// Ord-markering og tekst, klippet så teksten ikke flyter inn i gutteren ved sidescroll.
    /// Returnerer bredden av den bredeste linja.
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
                let caret;
                let number = if line == self.caret.line {
                    let color = self.theme.line_number_caret_row;
                    caret = shape_number(line, color, self.style, window);
                    &caret
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
