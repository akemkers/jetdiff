//! Én editor: bakgrunner, ord-markering, tekst, linjenumre og error stripe. Tegner bare
//! det `smudiff-view` gir, og bare synlige linjer.

use gpui::{
    App, Bounds, ContentMask, Hsla, Pixels, ShapedLine, SharedString, TextRun, Window, fill, point,
    px, quad, size,
};
use smudiff_core::side::Side;
use smudiff_view::columns::{self, ExpandedLine};
use smudiff_view::separator::{stroke_width, wave};
use smudiff_view::{FileDiff, Folding, SideGeometry, StripeMarker, thumb_range};

use crate::cache::LineCache;
use crate::divider::stroke_path;
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub struct CachedLine {
    pub expanded: ExpandedLine,
    /// `None` for tomme linjer, de har ingenting å shape.
    pub shaped: Option<ShapedLine>,
    pub number: ShapedLine,
}

impl CachedLine {
    /// X i den viste teksten for en byte-kolonne i originallinja.
    pub fn x_for_col(&self, col: usize) -> f32 {
        match &self.shaped {
            Some(s) => f32::from(s.x_for_index(self.expanded.display_index(col))),
            None => 0.,
        }
    }

    pub fn width(&self) -> f32 {
        self.shaped.as_ref().map_or(0., |s| f32::from(s.width))
    }
}

pub type SideCache = LineCache<(SharedString, u32), CachedLine>;

/// Error stripe-rektangler i sporet, slått sammen per piksel. Regnes ut når sporhøyden endres.
#[derive(Debug, Clone, Default)]
pub struct StripeRects {
    pub track_height: f32,
    pub rects: Vec<(f32, f32, Hsla)>,
}

impl StripeRects {
    pub fn new(
        markers: &[StripeMarker],
        content_height: f32,
        track_height: f32,
        theme: &UiTheme,
    ) -> Self {
        let mut rects: Vec<(f32, f32, Hsla)> = Vec::new();
        for m in markers {
            let (start, end) = m.track_range(content_height, track_height);
            let color = theme.diff(m.diff_type).stripe;
            match rects.last_mut() {
                Some(last) if last.2 == color && start <= last.1 => last.1 = last.1.max(end),
                _ => rects.push((start, end, color)),
            }
        }
        Self {
            track_height,
            rects,
        }
    }
}

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
    pub caret: Caret,
    pub stripe: &'a StripeRects,
}

/// Cursoren slik denne siden tegner den.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    pub line: usize,
    pub col: usize,
    /// Fylt blokk på aktiv side med fokus, ellers bare omriss som vim i et inaktivt vindu.
    pub solid: bool,
}

fn rect(x: std::ops::Range<f32>, y1: f32, y2: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(px(x.start), px(y1)),
        size(px(x.end - x.start), px(y2 - y1)),
    )
}

fn run(len: usize, style: &TextStyle, color: Hsla) -> [TextRun; 1] {
    [TextRun {
        len,
        font: style.font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }]
}

fn mask(bounds: Bounds<Pixels>) -> Option<ContentMask<Pixels>> {
    Some(ContentMask { bounds })
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
            window.paint_quad(fill(gutter_bounds, self.theme.gutter_bg));
            // New UI: 1px strek ved separatoren, males før diff-markørene så den skjules i endringer
            window.paint_quad(fill(
                rect(g.gutter_x(gl.separator..gl.separator + 1.), top, bottom),
                self.theme.indent_guide,
            ));

            // Caret row under diff-bakgrunnene, som i editorens lagrekkefølge
            if (first..=last_line).contains(&self.caret.line) {
                let y = screen_y(layout.line_top(self.caret.line));
                window.paint_quad(fill(rect(g.text.clone(), y, y + lh), self.theme.caret_row));
            }
            self.paint_block_backgrounds(first, last, window);
            self.paint_folds(first_visual..=last_visual, window);

            window.with_content_mask(mask(text_bounds), |window| {
                let text_x = g.text_x - self.scroll_x;
                for &line in &lines {
                    let y = screen_y(layout.line_top(line));
                    let entry = cache.get_or_insert_with(line, || self.shape(line, window));
                    widest = widest.max(entry.width());
                    for h in self.diff.line_highlight(side, line).inline {
                        let color = self.theme.diff(h.diff_type).background;
                        let x1 = text_x + entry.x_for_col(h.cols.start);
                        if h.cols.is_empty() && !h.past_line_end {
                            // `DiffEmptyHighlighterRenderer`
                            window.paint_quad(fill(rect(x1 - 1.0..x1 + 1., y, y + lh), color));
                            continue;
                        }
                        let x2 = if h.past_line_end {
                            g.text.end
                        } else {
                            text_x + entry.x_for_col(h.cols.end)
                        };
                        window.paint_quad(fill(rect(x1..x2, y, y + lh), color));
                    }
                    if let Some(shaped) = &entry.shaped {
                        let origin = point(px(text_x), px(y + self.style.text_dy));
                        let _ = shaped.paint(origin, px(lh), window, cx);
                    }
                }
                if (first_visual..=last_visual).contains(&layout.logical_to_visual(self.caret.line))
                {
                    self.paint_caret(cache, text_x, window, cx);
                }
            });

            window.with_content_mask(mask(gutter_bounds), |window| {
                for &line in &lines {
                    let y = screen_y(layout.line_top(line)) + self.style.number_dy;
                    let caret;
                    let number = if line == self.caret.line {
                        caret = self.shape_number(line, self.theme.line_number_caret_row, window);
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

            self.paint_stripe(layout.content_height(), window);
        });

        cache.trim(first..last_line + 1);
        widest
    }

    /// Blokk over tegnet. På en kollapset fold eller tom linje er den ett tegn bred ved
    /// tekststart.
    fn paint_caret(&self, cache: &mut SideCache, text_x: f32, window: &mut Window, cx: &mut App) {
        let c = self.caret;
        let side = self.geom.side;
        let lh = self.style.line_height();
        let layout = self.folding.layout(side, lh);
        let y = self.top + layout.line_top(c.line) - self.scroll_y;
        let folded = self.folding.fold_at_line(side, c.line).is_some();
        let text = self.diff.line(side, c.line);
        let next = columns::next_col(text, c.col);
        let (x1, x2, glyph) = if folded || next <= c.col {
            (text_x, text_x + self.style.char_width, None)
        } else {
            let entry = cache.get_or_insert_with(c.line, || self.shape(c.line, window));
            let (a, b) = (
                entry.expanded.display_index(c.col),
                entry.expanded.display_index(next),
            );
            let x1 = text_x + entry.x_for_col(c.col);
            let x2 = (text_x + entry.x_for_col(next)).max(x1 + 1.);
            let glyph = entry.expanded.text[a..b].trim_end().to_owned();
            (x1, x2, (!glyph.is_empty()).then_some(glyph))
        };
        let color = self.theme.caret;
        if !c.solid {
            for r in [
                rect(x1..x2, y, y + 1.),
                rect(x1..x2, y + lh - 1., y + lh),
                rect(x1..x1 + 1., y, y + lh),
                rect(x2 - 1.0..x2, y, y + lh),
            ] {
                window.paint_quad(fill(r, color));
            }
            return;
        }
        window.paint_quad(fill(rect(x1..x2, y, y + lh), color));
        // Tegnet under males på nytt i bakgrunnsfargen, som en blokk-caret i vim
        if let Some(glyph) = glyph {
            let text = SharedString::from(glyph);
            let runs = run(text.len(), self.style, self.theme.editor_bg);
            let shaped = window
                .text_system()
                .shape_line(text, self.style.font_size, &runs, None);
            let origin = point(px(x1), px(y + self.style.text_dy));
            let _ = shaped.paint(origin, px(lh), window, cx);
        }
    }

    /// `DiffLineSeparatorRenderer`: bølge over gutter og tekst, med fasen fra divideren.
    fn paint_folds(&self, visual: std::ops::RangeInclusive<usize>, window: &mut Window) {
        let g = self.geom;
        let lh = self.style.line_height();
        let layout = self.folding.layout(g.side, lh);
        let (edge, mirrored) = match g.side {
            Side::Left => (g.gutter.end, true),
            Side::Right => (g.gutter.start, false),
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
            let path = wave(g.bounds(), edge, mirrored, y, lh);
            let width = stroke_width(hovered == Some(id));
            let p = |pt: smudiff_view::Point| point(px(pt.x), px(pt.y));
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

    /// Scrollbaren ligger over teksten, så den males etter den.
    fn paint_stripe(&self, content_height: f32, window: &mut Window) {
        let g = self.geom;
        let markers = g.marker_x();
        if !markers.is_empty() {
            for &(y1, y2, color) in &self.stripe.rects {
                window.paint_quad(fill(
                    rect(markers.clone(), self.top + y1, self.top + y2),
                    color,
                ));
            }
        }
        let thumb = g.thumb_x();
        if thumb.is_empty() {
            return;
        }
        if let Some((y1, y2)) = thumb_range(self.scroll_y, self.height, content_height, self.height)
        {
            let radius = (thumb.end - thumb.start) / 2.;
            window.paint_quad(quad(
                rect(thumb, self.top + y1, self.top + y2),
                px(radius),
                self.theme.scrollbar_thumb,
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
        }
    }

    fn shape(&self, line: usize, window: &mut Window) -> CachedLine {
        let expanded = ExpandedLine::new(self.diff.line(self.geom.side, line));
        let shaped = (!expanded.text.is_empty()).then(|| {
            let text = SharedString::from(expanded.text.clone());
            let runs = run(text.len(), self.style, self.theme.editor_fg);
            window
                .text_system()
                .shape_line(text, self.style.font_size, &runs, None)
        });
        // Standardfargen forsvinner mot diff-bakgrunnen, spesielt blå
        let changed = self
            .diff
            .line_highlight(self.geom.side, line)
            .block
            .is_some();
        let color = if changed {
            self.theme.line_number_caret_row
        } else {
            self.theme.line_number
        };
        CachedLine {
            expanded,
            shaped,
            number: self.shape_number(line, color, window),
        }
    }

    fn shape_number(&self, line: usize, color: Hsla, window: &mut Window) -> ShapedLine {
        let text = SharedString::from((line + 1).to_string());
        let runs = run(text.len(), self.style, color);
        window
            .text_system()
            .shape_line(text, self.style.number_font_size, &runs, None)
    }
}
