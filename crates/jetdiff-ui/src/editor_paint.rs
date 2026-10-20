//! Tegning som split og unified deler: tekst med syntaks, ord-markering, linjenumre og
//! error stripe.

use std::ops::Range;

use gpui::{
    Bounds, ContentMask, Hsla, Pixels, ShapedLine, SharedString, TextRun, Window, fill, point, px,
    quad, size,
};
use jetdiff_viewmodel::columns::ExpandedLine;
use jetdiff_viewmodel::model::SyntaxSpan;
use jetdiff_viewmodel::{InlineHighlight, SideGeometry, StripeMarker, thumb_range};

use crate::text::TextStyle;
use crate::theme::{UiDiffColors, UiTheme};

pub fn rect(x: Range<f32>, top: f32, bottom: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(px(x.start), px(top)),
        size(px(x.end - x.start), px(bottom - top)),
    )
}

pub fn run(len: usize, style: &TextStyle, color: Hsla) -> TextRun {
    TextRun {
        len,
        font: style.font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

pub fn mask(bounds: Bounds<Pixels>) -> Option<ContentMask<Pixels>> {
    Some(ContentMask { bounds })
}

/// Bakgrunn for hele editoren, og 1px strek ved separatoren i gutteren.
pub fn paint_editor_background(
    geom: &SideGeometry,
    top: f32,
    bottom: f32,
    theme: &UiTheme,
    window: &mut Window,
) {
    let gutter = geom.gutter_layout;
    window.paint_quad(fill(rect(geom.bounds(), top, bottom), theme.editor_bg));
    window.paint_quad(fill(
        rect(geom.gutter.clone(), top, bottom),
        theme.editor_bg,
    ));
    let separator = geom.gutter_x(gutter.separator..gutter.separator + 1.);
    window.paint_quad(fill(rect(separator, top, bottom), theme.indent_guide));
}

/// Diff-farge for en endring: full farge i nummerområdet. Ytterste 3px av gutteren og hele
/// teksten får ignored-farge når endringen bare er whitespace.
pub fn paint_change(
    geom: &SideGeometry,
    top: f32,
    bottom: f32,
    colors: &UiDiffColors,
    ignored: bool,
    window: &mut Window,
) {
    let gutter = geom.gutter_layout;
    let separator = geom.gutter_x(gutter.separator..gutter.width);
    let outer = separator.start.min(geom.text.start)..separator.end.max(geom.text.end);
    let text_color = if ignored {
        colors.ignored
    } else {
        colors.background
    };
    let numbers = geom.gutter_x(0.0..gutter.separator);
    window.paint_quad(fill(rect(numbers, top, bottom), colors.background));
    window.paint_quad(fill(rect(outer, top, bottom), text_color));
}

/// Markert linje, over diff-bakgrunnene så den ikke forsvinner i endringer. Med fokus
/// markeres også nummeret, som er lettest å finne igjen etter et hopp.
pub fn paint_caret_row(
    geom: &SideGeometry,
    top: f32,
    bottom: f32,
    active: bool,
    theme: &UiTheme,
    window: &mut Window,
) {
    let color = if active {
        theme.row_highlight
    } else {
        theme.row_highlight.opacity(0.4)
    };
    window.paint_quad(fill(rect(geom.text.clone(), top, bottom), color));
    if active {
        let numbers = geom.gutter_x(0.0..geom.gutter_layout.separator);
        window.paint_quad(fill(rect(numbers, top, bottom), theme.gutter_highlight));
    }
}

/// Én linje shapet med syntaksfarger og tabs ekspandert.
pub struct ShapedText {
    pub expanded: ExpandedLine,
    /// `None` for tomme linjer, de har ingenting å shape.
    pub shaped: Option<ShapedLine>,
}

impl ShapedText {
    pub fn new(
        text: &str,
        syntax: &[SyntaxSpan],
        style: &TextStyle,
        theme: &UiTheme,
        window: &mut Window,
    ) -> Self {
        let expanded = ExpandedLine::new(text);
        let shaped = (!expanded.text.is_empty()).then(|| {
            let text = SharedString::from(expanded.text.clone());
            let runs = syntax_runs(syntax, &expanded, style, theme);
            window
                .text_system()
                .shape_line(text, style.font_size, &runs, None)
        });
        Self { expanded, shaped }
    }

    /// X i den viste teksten for en byte-kolonne i originallinja.
    pub fn x_for_col(&self, col: usize) -> f32 {
        match &self.shaped {
            Some(shaped) => f32::from(shaped.x_for_index(self.expanded.display_index(col))),
            None => 0.,
        }
    }

    pub fn width(&self) -> f32 {
        self.shaped
            .as_ref()
            .map_or(0., |shaped| f32::from(shaped.width))
    }

    /// Ord-markering for linja. `text_x` er der teksten starter etter horisontal scroll,
    /// `text_end` er høyre kant av tekstområdet og `row` er linjas y-område.
    pub fn paint_inline(
        &self,
        highlights: &[InlineHighlight],
        text_x: f32,
        text_end: f32,
        row: Range<f32>,
        theme: &UiTheme,
        window: &mut Window,
    ) {
        for highlight in highlights {
            let color = theme.diff(highlight.diff_type).background;
            let start_x = text_x + self.x_for_col(highlight.cols.start);
            if highlight.cols.is_empty() && !highlight.past_line_end {
                // `DiffEmptyHighlighterRenderer`
                let bounds = rect(start_x - 1.0..start_x + 1., row.start, row.end);
                window.paint_quad(fill(bounds, color));
                continue;
            }
            let end_x = if highlight.past_line_end {
                text_end
            } else {
                text_x + self.x_for_col(highlight.cols.end)
            };
            window.paint_quad(fill(rect(start_x..end_x, row.start, row.end), color));
        }
    }
}

/// Runs for teksten med tabs ekspandert. Hull mellom tokens får standardfargen.
fn syntax_runs(
    syntax: &[SyntaxSpan],
    expanded: &ExpandedLine,
    style: &TextStyle,
    theme: &UiTheme,
) -> Vec<TextRun> {
    let plain = |len| run(len, style, theme.editor_fg);
    let mut runs = Vec::new();
    let mut at = 0;
    for span in syntax {
        let start = expanded.display_index(span.cols.start);
        let end = expanded.display_index(span.cols.end);
        if start > at {
            runs.push(plain(start - at));
        }
        let syntax_style = theme.syntax(span.token);
        runs.push(TextRun {
            font: syntax_style.font(&style.font),
            color: syntax_style.color,
            ..plain(end - start)
        });
        at = end;
    }
    if expanded.text.len() > at {
        runs.push(plain(expanded.text.len() - at));
    }
    runs
}

/// Linjenummeret til en linje som telles fra 0.
pub fn shape_number(
    line: usize,
    color: Hsla,
    style: &TextStyle,
    window: &mut Window,
) -> ShapedLine {
    let text = SharedString::from((line + 1).to_string());
    let runs = [run(text.len(), style, color)];
    window
        .text_system()
        .shape_line(text, style.number_font_size, &runs, None)
}

/// Error stripe-rektangler i sporet, slått sammen per piksel. Regnes ut når sporhøyden endres.
#[derive(Debug, Clone, Default)]
pub struct StripeRects {
    /// Hele høyden til editoren.
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
        for marker in markers {
            let (start, end) = marker.track_range(content_height, track_height);
            let color = theme.diff(marker.diff_type).stripe;
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

    /// Markørene og scrollbar-tommelen. Scrollbaren ligger over teksten, så den males etter den.
    pub fn paint(
        &self,
        geom: &SideGeometry,
        top: f32,
        scroll_y: f32,
        content_height: f32,
        theme: &UiTheme,
        window: &mut Window,
    ) {
        let markers = geom.marker_x();
        if !markers.is_empty() {
            for &(start, end, color) in &self.rects {
                window.paint_quad(fill(rect(markers.clone(), top + start, top + end), color));
            }
        }
        let thumb = geom.thumb_x();
        if thumb.is_empty() {
            return;
        }
        let height = self.track_height;
        if let Some((start, end)) = thumb_range(scroll_y, height, content_height, height) {
            let radius = (thumb.end - thumb.start) / 2.;
            window.paint_quad(quad(
                rect(thumb, top + start, top + end),
                px(radius),
                theme.scrollbar_thumb,
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
        }
    }
}
