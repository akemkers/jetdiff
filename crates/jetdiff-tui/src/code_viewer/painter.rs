//! The building blocks Split, Unified and the scrollbar draw with. Only this file knows
//! the state, the theme and the buffer.

use jetdiff_viewmodel::{FileDiff, HunkKind, Row, Side, StripeMarker, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::hunk_color::HunkColor;
use super::row_at;
use crate::color::{color, faded, over};
use crate::geometry::{self, Columns};
use crate::text::{LineText, fill, paint_line, tint};

/// The cursor's row when the diff doesn't have focus, as in GPUI.
const UNFOCUSED_CARET: f32 = 0.4;

pub(super) struct Painter<'a> {
    state: &'a UnifiedState,
    theme: &'a Theme,
    focused: bool,
    buf: &'a mut Buffer,
}

/// A visible row, and the screen row it is on.
pub(super) struct VisibleRow<'a> {
    pub(super) y: u16,
    pub(super) row: &'a Row,
    pub(super) marked: bool,
}

impl<'a> Painter<'a> {
    pub(super) fn new(
        state: &'a UnifiedState,
        theme: &'a Theme,
        focused: bool,
        buf: &'a mut Buffer,
    ) -> Self {
        Self {
            state,
            theme,
            focused,
            buf,
        }
    }

    pub(super) fn diff(&self) -> &'a FileDiff {
        &self.state.diff.diff
    }

    pub(super) fn visible_rows(
        &self,
        area: Rect,
    ) -> impl Iterator<Item = VisibleRow<'a>> + use<'a> {
        let state = self.state;
        let layout = state.diff.layout(1.);
        let scroll_y = state.scroll_y();
        (0..area.height).filter_map(move |screen| {
            let index = row_at(&layout, scroll_y, screen)?;
            Some(VisibleRow {
                y: area.y + screen,
                row: state.diff.row(index),
                marked: index == state.row,
            })
        })
    }

    pub(super) fn markers(&self) -> Vec<StripeMarker> {
        self.state.diff.stripe_markers(1.)
    }

    pub(super) fn content_height(&self) -> f32 {
        self.state.diff.layout(1.).content_height()
    }

    pub(super) fn scroll_y(&self) -> f32 {
        self.state.scroll_y()
    }

    pub(super) fn background(&mut self, area: Rect) {
        fill(area, color(self.theme.editor_bg), self.buf);
    }

    /// The background of the hunk and the text with syntax and inline highlights. `inline_highlight_kind` gives all
    /// the words the same color; otherwise each word gets its own.
    pub(super) fn line(
        &mut self,
        columns: &Columns,
        y: u16,
        side: Side,
        line: usize,
        hunk_color: Option<HunkColor>,
        inline_highlight_kind: Option<HunkKind>,
    ) {
        if let Some(hunk_color) = hunk_color {
            let colors = self.theme.diff(hunk_color.kind);
            let text = if hunk_color.compared_by_word {
                colors.ignored
            } else {
                colors.background
            };
            fill(columns.gutter(y), color(colors.background), self.buf);
            fill(columns.text(y), color(text), self.buf);
        }
        let diff = self.diff();
        let content = LineText {
            text: diff.line(side, line),
            syntax: diff.line_syntax(side, line),
            inline: diff.line_highlight(side, line).inline,
            inline_highlight_kind,
        };
        let scroll_x = self.state.scroll.x.round() as usize;
        paint_line(&content, scroll_x, columns.text(y), self.theme, self.buf);
    }

    /// Right-aligned to `end`. The default color disappears against the diff background, so hunk and
    /// cursor rows get a stronger one.
    pub(super) fn line_number(
        &mut self,
        end: u16,
        y: u16,
        line: usize,
        hunk_color: Option<HunkColor>,
        marked: bool,
    ) {
        let number_color = if hunk_color.is_some() || marked {
            self.theme.line_number_caret_row
        } else {
            self.theme.line_number
        };
        let (x, number) = geometry::number(line, end);
        self.buf
            .set_string(x, y, number, Style::default().fg(color(number_color)));
    }

    /// The side has no line on the row.
    pub(super) fn filler(&mut self, columns: &Columns, y: u16) {
        let area = columns.row(y);
        let style = Style::default()
            .fg(color(self.theme.indent_guide))
            .bg(color(self.theme.editor_bg));
        let hatch = "╱".repeat(area.width as usize);
        self.buf
            .set_stringn(area.x, area.y, hatch, usize::MAX, style);
    }

    /// The cursor's row. Fainter when the diff doesn't have focus.
    pub(super) fn caret(&mut self, columns: &Columns, y: u16) {
        let highlight = if self.focused {
            self.theme.row_highlight
        } else {
            faded(self.theme.row_highlight, UNFOCUSED_CARET)
        };
        tint(columns.text(y), highlight, self.buf);
        if self.focused {
            let style = Style::default().bg(color(self.theme.gutter_highlight));
            self.buf.set_style(columns.gutter(y), style);
        }
    }

    pub(super) fn divider(&mut self, x: u16, area: Rect) {
        let style = Style::default().fg(color(self.theme.indent_guide));
        for y in area.top()..area.bottom() {
            self.buf.set_string(x, y, "│", style);
        }
    }

    /// One cell in the scrollbar, with the thumb and the hunk minimap mark for a hunk.
    pub(super) fn scrollbar_cell(
        &mut self,
        x: u16,
        y: u16,
        in_thumb: bool,
        marker: Option<HunkKind>,
    ) {
        let bg = color(self.theme.editor_bg);
        let cell = &mut self.buf[(x, y)];
        cell.set_bg(if in_thumb {
            over(self.theme.scrollbar_thumb, bg)
        } else {
            bg
        });
        match marker {
            Some(kind) => {
                let marker_color = brighter(color(self.theme.diff(kind).background));
                cell.set_char('▐').set_fg(marker_color);
            }
            None => {
                cell.set_char(' ');
            }
        }
    }
}

/// The diff background is too faint as a hunk minimap mark.
fn brighter(base: Color) -> Color {
    over([255, 255, 255, 60], base)
}
