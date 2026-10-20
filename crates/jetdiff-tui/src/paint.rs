//! Split og unified. Scroll rundes til hele rader.

use jetdiff_viewmodel::stripe::thumb_range;
use jetdiff_viewmodel::{
    DiffState, FileDiff, LineLayout, Side, StripeMarker, Theme, UnifiedState, stripe_markers,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::color::{color, diff_colors, faded, over};
use crate::geometry::{self, DIVIDER_WIDTH, SideCells};
use crate::text::{LineText, fill, paint_line, tint};

const SIDES: [Side; 2] = [Side::Left, Side::Right];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowContent {
    Line(usize),
    Fold,
    Empty,
}

pub fn row_content(layout: &LineLayout, scroll_y: f32, row: u16) -> RowContent {
    let visual = layout.y_to_visual_line(scroll_y.round() + row as f32);
    if visual >= layout.visual_line_count() {
        RowContent::Empty
    } else if layout.fold_at_visual(visual).is_some() {
        RowContent::Fold
    } else {
        RowContent::Line(layout.visual_to_logical(visual))
    }
}

fn screen_row(y: f32, scroll_y: f32) -> f32 {
    y - scroll_y.round()
}

pub struct Caret {
    pub line: usize,
    pub active: bool,
}

pub fn paint_split(state: &DiffState, focused: bool, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let sides = geometry::split(&state.diff, area);
    let other = state.transferred_cursor();
    for (cells, side) in sides.iter().zip(SIDES) {
        let caret = if side == state.cursor.side {
            state.cursor
        } else {
            other
        };
        let caret = Caret {
            line: caret.line,
            active: focused && side == state.cursor.side,
        };
        let scroll_y = state.scroll.y(side);
        let layout = state.folding.layout(side, 1.);
        paint_side(
            &state.diff,
            &layout,
            cells,
            scroll_y,
            state.scroll.x,
            &caret,
            area,
            theme,
            buf,
        );
        let markers = stripe_markers(&state.diff, &state.folding, side, 1.);
        paint_scrollbar(
            cells.scrollbar,
            &markers,
            &layout,
            scroll_y,
            area,
            theme,
            buf,
        );
    }
    let x = sides[0].gutter.end;
    let divider = Rect {
        x,
        width: DIVIDER_WIDTH.min(area.right() - x),
        ..area
    };
    crate::divider::paint(state, divider, theme, buf);
}

#[allow(clippy::too_many_arguments)]
fn paint_side(
    diff: &FileDiff,
    layout: &LineLayout,
    cells: &SideCells,
    scroll_y: f32,
    scroll_x: f32,
    caret: &Caret,
    area: Rect,
    theme: &Theme,
    buf: &mut Buffer,
) {
    let side = cells.side;
    let bg = color(theme.editor_bg);
    for row in 0..area.height {
        let y = area.y + row;
        let gutter = Rect::new(
            cells.gutter.start,
            y,
            cells.gutter.end - cells.gutter.start,
            1,
        );
        let text = Rect::new(cells.text.start, y, cells.text_width(), 1);
        fill(gutter.union(text), bg, buf);
        match row_content(layout, scroll_y, row) {
            RowContent::Empty => {}
            RowContent::Fold => {
                let wave = Style::default().fg(color(theme.separator_wave));
                let line = "┄".repeat((gutter.union(text)).width as usize);
                buf.set_stringn(gutter.union(text).x, y, line, usize::MAX, wave);
            }
            RowContent::Line(line) => {
                let highlight = diff.line_highlight(side, line);
                let block = highlight.block.map(|b| &diff.blocks()[b]);
                if let Some(block) = block {
                    let colors = diff_colors(theme, block.diff_type);
                    fill(gutter, color(colors.background), buf);
                    let text_bg = if block.ignored() {
                        colors.ignored
                    } else {
                        colors.background
                    };
                    fill(text, color(text_bg), buf);
                }
                let content = LineText {
                    text: diff.line(side, line),
                    syntax: diff.line_syntax(side, line),
                    inline: highlight.inline,
                    inline_type: None,
                };
                paint_line(&content, scroll_x.round() as usize, text, theme, buf);
                let number_color = if block.is_some() || line == caret.line {
                    theme.line_number_caret_row
                } else {
                    theme.line_number
                };
                let (x, number) = cells.number(line);
                buf.set_string(x, y, number, Style::default().fg(color(number_color)));
                if line == caret.line {
                    paint_caret(gutter, text, caret.active, theme, buf);
                }
            }
        }
    }
    paint_empty_blocks(diff, layout, cells, scroll_y, area, theme, buf);
}

fn paint_caret(gutter: Rect, text: Rect, active: bool, theme: &Theme, buf: &mut Buffer) {
    let highlight = if active {
        theme.row_highlight
    } else {
        faded(theme.row_highlight, 0.4)
    };
    tint(text, highlight, buf);
    if active {
        let numbers = Rect {
            x: gutter.x,
            width: gutter.width,
            ..gutter
        };
        buf.set_style(numbers, Style::default().bg(color(theme.gutter_highlight)));
    }
}

/// Terminalen har ikke halve rader, så en endring uten linjer på denne siden er en
/// understreking av linja over.
fn paint_empty_blocks(
    diff: &FileDiff,
    layout: &LineLayout,
    cells: &SideCells,
    scroll_y: f32,
    area: Rect,
    theme: &Theme,
    buf: &mut Buffer,
) {
    let side = cells.side;
    let x = cells.gutter.start.min(cells.text.start);
    let width = cells.gutter.end.max(cells.text.end) - x;
    for block in diff.blocks() {
        let lines = block.lines(side);
        if !lines.is_empty() || lines.start == 0 {
            continue;
        }
        let row = screen_row(layout.line_top(lines.start - 1), scroll_y);
        if row < 0. || row >= area.height as f32 {
            continue;
        }
        let line = color(diff_colors(theme, block.diff_type).background);
        let style = Style::default()
            .underline_color(brighter(line))
            .add_modifier(ratatui::style::Modifier::UNDERLINED);
        buf.set_style(Rect::new(x, area.y + row as u16, width, 1), style);
    }
}

/// Diff-bakgrunnen er for svak som strek og markør.
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
        let cell_bg = if in_thumb {
            over(theme.scrollbar_thumb, bg)
        } else {
            bg
        };
        let marker = markers.iter().find(|m| {
            let (s, e) = (
                to_track(m.start_y),
                to_track(m.end_y).max(to_track(m.start_y) + 1.),
            );
            s < bottom && e > top
        });
        let cell = &mut buf[(x, area.y + row)];
        cell.set_bg(cell_bg);
        match marker {
            Some(m) => {
                cell.set_char('▐');
                cell.set_fg(brighter(color(diff_colors(theme, m.diff_type).background)));
            }
            None => {
                cell.set_char(' ');
            }
        }
    }
}

pub fn paint_unified(
    state: &UnifiedState,
    focused: bool,
    area: Rect,
    theme: &Theme,
    buf: &mut Buffer,
) {
    let unified = &state.diff;
    let diff = &unified.diff;
    let cells = geometry::unified(diff, area);
    let editor = &cells.editor;
    let layout = unified.layout(1.);
    let scroll_y = state.scroll_y();
    let bg = color(theme.editor_bg);
    for screen in 0..area.height {
        let y = area.y + screen;
        let gutter = Rect::new(
            editor.gutter.start,
            y,
            editor.gutter.end - editor.gutter.start,
            1,
        );
        let text = Rect::new(editor.text.start, y, editor.text_width(), 1);
        fill(gutter.union(text), bg, buf);
        let RowContent::Line(index) = row_content(&layout, scroll_y, screen) else {
            continue;
        };
        let row = unified.row(index);
        if let Some(block) = row.block {
            let colors = diff_colors(theme, row.diff_type());
            fill(gutter, color(colors.background), buf);
            let ignored = diff.blocks()[block].ignored();
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
            text: unified.text(index),
            syntax: diff.line_syntax(row.side(), row.line()),
            inline: diff.line_highlight(row.side(), row.line()).inline,
            inline_type: Some(row.diff_type()),
        };
        paint_line(&content, state.scroll.x.round() as usize, text, theme, buf);
        let number_color = if row.block.is_some() || index == state.row {
            theme.line_number_caret_row
        } else {
            theme.line_number
        };
        for (x, number) in cells.numbers(row.lines).into_iter().flatten() {
            buf.set_string(x, y, number, Style::default().fg(color(number_color)));
        }
        if index == state.row {
            paint_caret(gutter, text, focused, theme, buf);
        }
    }
    let markers = unified.stripe_markers(1.);
    paint_scrollbar(
        editor.scrollbar,
        &markers,
        &layout,
        scroll_y,
        area,
        theme,
        buf,
    );
}
