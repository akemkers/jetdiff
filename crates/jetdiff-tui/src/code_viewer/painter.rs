//! Byggeklossene split, unified og scrollbaren tegner med. Bare denne fila kjenner
//! tilstanden, temaet og bufferen.

use jetdiff_viewmodel::{DiffType, FileDiff, Row, Side, StripeMarker, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::change::Change;
use super::row_at;
use crate::color::{color, faded, over};
use crate::geometry::{self, Columns};
use crate::text::{LineText, fill, paint_line, tint};

/// Markert rad når diffen ikke har fokus, som i GPUI.
const UNFOCUSED_CARET: f32 = 0.4;

pub(super) struct Painter<'a> {
    state: &'a UnifiedState,
    theme: &'a Theme,
    focused: bool,
    buf: &'a mut Buffer,
}

/// En rad som synes, og skjermraden den står på.
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

    /// Bakgrunnen til endringen og teksten med syntaks og ord-markering. `word_type` gir alle
    /// ordene samme farge, ellers får hvert ord sin egen.
    pub(super) fn line(
        &mut self,
        columns: &Columns,
        y: u16,
        side: Side,
        line: usize,
        change: Option<Change>,
        word_type: Option<DiffType>,
    ) {
        if let Some(change) = change {
            let colors = self.theme.diff(change.diff_type);
            let text = if change.ignored {
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
            inline_type: word_type,
        };
        let scroll_x = self.state.scroll.x.round() as usize;
        paint_line(&content, scroll_x, columns.text(y), self.theme, self.buf);
    }

    /// Høyrejustert mot `end`. Standardfargen forsvinner mot diff-bakgrunnen, så endrede og
    /// markerte rader får en sterkere.
    pub(super) fn line_number(
        &mut self,
        end: u16,
        y: u16,
        line: usize,
        change: Option<Change>,
        marked: bool,
    ) {
        let number_color = if change.is_some() || marked {
            self.theme.line_number_caret_row
        } else {
            self.theme.line_number
        };
        let (x, number) = geometry::number(line, end);
        self.buf
            .set_string(x, y, number, Style::default().fg(color(number_color)));
    }

    /// Siden har ingen linje på raden.
    pub(super) fn filler(&mut self, columns: &Columns, y: u16) {
        let area = columns.row(y);
        let style = Style::default()
            .fg(color(self.theme.indent_guide))
            .bg(color(self.theme.editor_bg));
        let hatch = "╱".repeat(area.width as usize);
        self.buf
            .set_stringn(area.x, area.y, hatch, usize::MAX, style);
    }

    /// Markert rad. Svakere når diffen ikke har fokus.
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

    /// Én celle i scrollbaren, med tommel og markør for en endring.
    pub(super) fn scrollbar_cell(
        &mut self,
        x: u16,
        y: u16,
        in_thumb: bool,
        marker: Option<DiffType>,
    ) {
        let bg = color(self.theme.editor_bg);
        let cell = &mut self.buf[(x, y)];
        cell.set_bg(if in_thumb {
            over(self.theme.scrollbar_thumb, bg)
        } else {
            bg
        });
        match marker {
            Some(diff_type) => {
                let marker_color = brighter(color(self.theme.diff(diff_type).background));
                cell.set_char('▐').set_fg(marker_color);
            }
            None => {
                cell.set_char(' ');
            }
        }
    }
}

/// Diff-bakgrunnen er for svak som markør.
fn brighter(base: Color) -> Color {
    over([255, 255, 255, 60], base)
}
