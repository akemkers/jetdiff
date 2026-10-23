//! Drawing that Split and Unified share: text with syntax, inline highlights, line numbers and
//! the hunk minimap.

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

/// The mirrored cursor, or the cursor when the diff doesn't have focus.
const INACTIVE_CURSOR_ROW_OPACITY: f32 = 0.4;

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

/// Background for the whole side or for Unified, and a 1px stroke at the separator in the gutter.
pub fn paint_code_background(
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

/// Diff color for a hunk: full color in the number area. The outermost 3px of the gutter and all
/// of the text get IntelliJ's ignored color when the hunk was compared by word.
pub fn paint_hunk(
    geom: &SideGeometry,
    top: f32,
    bottom: f32,
    colors: &UiDiffColors,
    compared_by_word: bool,
    window: &mut Window,
) {
    let gutter = geom.gutter_layout;
    let separator = geom.gutter_x(gutter.separator..gutter.width);
    let outer = separator.start.min(geom.text.start)..separator.end.max(geom.text.end);
    let text_color = if compared_by_word {
        colors.ignored
    } else {
        colors.background
    };
    let numbers = geom.gutter_x(0.0..gutter.separator);
    window.paint_quad(fill(rect(numbers, top, bottom), colors.background));
    window.paint_quad(fill(rect(outer, top, bottom), text_color));
}

/// The cursor's line, over the diff backgrounds so it doesn't disappear in hunks. With focus
/// the number is also highlighted, which is easiest to find again after a jump.
pub fn paint_cursor_row(
    geom: &SideGeometry,
    top: f32,
    bottom: f32,
    active: bool,
    theme: &UiTheme,
    window: &mut Window,
) {
    let color = if active {
        theme.cursor_row_background
    } else {
        theme
            .cursor_row_background
            .opacity(INACTIVE_CURSOR_ROW_OPACITY)
    };
    window.paint_quad(fill(rect(geom.text.clone(), top, bottom), color));
    if active {
        let numbers = geom.gutter_x(0.0..geom.gutter_layout.separator);
        window.paint_quad(fill(rect(numbers, top, bottom), theme.cursor_row_gutter));
    }
}

/// One line shaped with syntax colors and tabs expanded.
pub struct ShapedText {
    pub expanded: ExpandedLine,
    /// `None` for empty lines; they have nothing to shape.
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

    /// X in the shown text for a byte column in the original line.
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

    /// Inline highlight for the line. `text_x` is where the text starts after horizontal scroll,
    /// `text_end` is the right edge of the text area and `row` is the line's y range.
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
            let color = theme.diff(highlight.kind).background;
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

/// Runs for the text with tabs expanded. Holes between tokens get the default color.
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

/// The line number of a line counted from 0.
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

/// Hunk minimap rectangles in the track, merged per pixel. Recomputed when the track height changes.
#[derive(Debug, Clone, Default)]
pub struct StripeRects {
    /// The whole height of the code view.
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
            let color = theme.diff(marker.kind).stripe;
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

    /// The hunk minimap and the scrollbar thumb. The scrollbar lies over the text, so it is painted after it.
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
