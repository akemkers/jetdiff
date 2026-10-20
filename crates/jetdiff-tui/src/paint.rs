//! Split og unified. Begge er rader fra `UnifiedDiff`, og scroll rundes til hele rader.

use jetdiff_viewmodel::stripe::thumb_range;
use jetdiff_viewmodel::{DiffType, FileDiff, LineLayout, Side, StripeMarker, Theme, UnifiedState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::color::{color, diff_colors, faded, over};
use crate::geometry::{self, Columns};
use crate::text::{LineText, fill, paint_line, tint};

/// Raden på skjermraden `screen`, `None` under siste rad.
pub fn row_at(layout: &LineLayout, scroll_y: f32, screen: u16) -> Option<usize> {
    let row = layout.y_to_logical_line(scroll_y.round() + screen as f32);
    (row < layout.line_count()).then_some(row)
}

pub fn paint(state: &UnifiedState, focused: bool, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let unified = &state.diff;
    let diff = &unified.diff;
    let layout = unified.layout(1.);
    let scroll_y = state.scroll_y();
    let scroll_x = state.scroll.x.round() as usize;
    fill(area, color(theme.editor_bg), buf);
    let scrollbar = if unified.is_aligned() {
        let cells = geometry::aligned(diff, area);
        let guide = Style::default().fg(color(theme.indent_guide));
        for screen in 0..area.height {
            let y = area.y + screen;
            buf.set_string(cells.divider, y, "│", guide);
            let Some(index) = row_at(&layout, scroll_y, screen) else {
                continue;
            };
            let row = unified.row(index);
            let diff_type = row.block.map(|b| diff.blocks()[b].diff_type);
            for (columns, side) in cells.sides.iter().zip([Side::Left, Side::Right]) {
                let line = row.lines[side.select(0, 1)];
                let painted = Painted {
                    diff,
                    columns,
                    y,
                    block: row.block,
                    diff_type,
                };
                match line {
                    Some(line) => {
                        painted.line(side, line, None, scroll_x, theme, buf);
                        let (x, number) = columns.number(line, columns.number_end());
                        painted.number(x, number, index == state.row, theme, buf);
                    }
                    None => filler(columns.row(y), theme, buf),
                }
                if index == state.row {
                    caret(columns, y, focused, theme, buf);
                }
            }
        }
        cells.scrollbar
    } else {
        let cells = geometry::unified(diff, area);
        let columns = &cells.columns;
        for screen in 0..area.height {
            let y = area.y + screen;
            let Some(index) = row_at(&layout, scroll_y, screen) else {
                continue;
            };
            let row = unified.row(index);
            let painted = Painted {
                diff,
                columns,
                y,
                block: row.block,
                diff_type: row.block.map(|_| row.diff_type()),
            };
            let inline_type = Some(row.diff_type());
            painted.line(row.side(), row.line(), inline_type, scroll_x, theme, buf);
            let ends = [cells.old_end, columns.number_end()];
            for (line, end) in row.lines.into_iter().zip(ends) {
                if let Some(line) = line {
                    let (x, number) = columns.number(line, end);
                    painted.number(x, number, index == state.row, theme, buf);
                }
            }
            if index == state.row {
                caret(columns, y, focused, theme, buf);
            }
        }
        cells.scrollbar
    };
    let markers = unified.stripe_markers(1.);
    paint_scrollbar(scrollbar, &markers, &layout, scroll_y, area, theme, buf);
}

/// Én side av én rad.
struct Painted<'a> {
    diff: &'a FileDiff,
    columns: &'a Columns,
    y: u16,
    block: Option<usize>,
    diff_type: Option<DiffType>,
}

impl Painted<'_> {
    fn line(
        &self,
        side: Side,
        line: usize,
        inline_type: Option<DiffType>,
        scroll_x: usize,
        theme: &Theme,
        buf: &mut Buffer,
    ) {
        let (gutter, text) = (self.columns.gutter(self.y), self.columns.text(self.y));
        if let (Some(block), Some(diff_type)) = (self.block, self.diff_type) {
            let colors = diff_colors(theme, diff_type);
            fill(gutter, color(colors.background), buf);
            let ignored = self.diff.blocks()[block].ignored();
            fill(
                text,
                color(if ignored {
                    colors.ignored
                } else {
                    colors.background
                }),
                buf,
            );
        }
        let content = LineText {
            text: self.diff.line(side, line),
            syntax: self.diff.line_syntax(side, line),
            inline: self.diff.line_highlight(side, line).inline,
            inline_type,
        };
        paint_line(&content, scroll_x, text, theme, buf);
    }

    /// Standardfargen forsvinner mot diff-bakgrunnen.
    fn number(&self, x: u16, number: String, caret: bool, theme: &Theme, buf: &mut Buffer) {
        let c = if self.block.is_some() || caret {
            theme.line_number_caret_row
        } else {
            theme.line_number
        };
        buf.set_string(x, self.y, number, Style::default().fg(color(c)));
    }
}

/// Siden har ingen linje på raden.
fn filler(area: Rect, theme: &Theme, buf: &mut Buffer) {
    let style = Style::default()
        .fg(color(theme.indent_guide))
        .bg(color(theme.editor_bg));
    buf.set_stringn(
        area.x,
        area.y,
        "╱".repeat(area.width as usize),
        usize::MAX,
        style,
    );
}

fn caret(columns: &Columns, y: u16, active: bool, theme: &Theme, buf: &mut Buffer) {
    let highlight = if active {
        theme.row_highlight
    } else {
        faded(theme.row_highlight, 0.4)
    };
    tint(columns.text(y), highlight, buf);
    if active {
        let style = Style::default().bg(color(theme.gutter_highlight));
        buf.set_style(columns.gutter(y), style);
    }
}

/// Diff-bakgrunnen er for svak som markør.
fn brighter(c: Color) -> Color {
    over([255, 255, 255, 60], c)
}

fn paint_scrollbar(
    x: u16,
    markers: &[StripeMarker],
    layout: &LineLayout,
    scroll_y: f32,
    area: Rect,
    theme: &Theme,
    buf: &mut Buffer,
) {
    let track = area.height as f32;
    let content = layout.content_height();
    let to_track = |y: f32| {
        if content <= track {
            y
        } else {
            y / content * track
        }
    };
    let bg = color(theme.editor_bg);
    let thumb = thumb_range(scroll_y, track, content, track);
    for row in 0..area.height {
        let (top, bottom) = (row as f32, row as f32 + 1.);
        let in_thumb = thumb.is_some_and(|(s, e)| s < bottom && e > top);
        let marker = markers.iter().find(|m| {
            let start = to_track(m.start_y);
            start < bottom && to_track(m.end_y).max(start + 1.) > top
        });
        let cell = &mut buf[(x, area.y + row)];
        cell.set_bg(if in_thumb {
            over(theme.scrollbar_thumb, bg)
        } else {
            bg
        });
        match marker {
            Some(m) => {
                let marker = brighter(color(diff_colors(theme, m.diff_type).background));
                cell.set_char('▐').set_fg(marker);
            }
            None => {
                cell.set_char(' ');
            }
        }
    }
}
