//! The path, the keymap bar, the message, the help and the rollback dialog.

use jetdiff_app::help::{ALL, BAR};
use jetdiff_app::{Level, Notice};
use jetdiff_viewmodel::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::color::color;

const HELP_WIDTH: u16 = 42;
const CONFIRM_WIDTH: u16 = 28;
/// The keys in the help are in a column, with the description on the right.
const KEY_COLUMN: usize = 15;

fn text_row(text: &str, style: Style, area: Rect, buf: &mut Buffer) {
    Clear.render(area, buf);
    buf.set_style(area, style);
    buf.set_stringn(
        area.x + 1,
        area.y,
        text,
        area.width.saturating_sub(1) as usize,
        style,
    );
}

pub fn header(path: &str, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let style = Style::default()
        .fg(color(theme.editor_fg))
        .bg(color(theme.editor_bg));
    text_row(path, style, area, buf);
}

pub fn key_bar(area: Rect, theme: &Theme, buf: &mut Buffer) {
    let keys = Style::default().fg(color(theme.editor_fg));
    let what = Style::default().fg(color(theme.line_number));
    let mut spans = vec![Span::raw(" ")];
    for (key, description) in BAR {
        spans.push(Span::styled(key, keys));
        spans.push(Span::styled(format!(" {description}   "), what));
    }
    Clear.render(area, buf);
    buf.set_style(area, Style::default().bg(color(theme.editor_bg)));
    buf.set_line(area.x, area.y, &Line::from(spans), area.width);
}

pub fn message(notice: &Notice, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let bg = match notice.level {
        Level::Info => theme.message_bg,
        Level::Warning => theme.warning_bg,
    };
    let style = Style::default().fg(color(theme.message_fg)).bg(color(bg));
    text_row(&notice.text, style, area, buf);
}

/// The lines in a frame in the middle of `area`, with one column of space on the left.
fn popup(width: u16, lines: Vec<Line>, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let width = width.min(area.width);
    let height = (lines.len() as u16 + 2).min(area.height);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    Clear.render(rect, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme.line_number)))
        .style(Style::default().bg(color(theme.editor_bg)));
    let inner = block.inner(rect);
    block.render(rect, buf);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        ..inner
    };
    Paragraph::new(lines).render(inner, buf);
}

pub fn help(area: Rect, theme: &Theme, buf: &mut Buffer) {
    let keys = Style::default().fg(color(theme.editor_fg));
    let what = Style::default().fg(color(theme.line_number));
    let title = keys.add_modifier(Modifier::BOLD);
    let mut lines = Vec::new();
    for (section, rows) in ALL {
        lines.push(Line::styled(section, title));
        for (key, description) in rows {
            lines.push(Line::from(vec![
                Span::styled(format!("{key:<KEY_COLUMN$}"), keys),
                Span::styled(*description, what),
            ]));
        }
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled("? or Esc closes", what));
    popup(HELP_WIDTH, lines, area, theme, buf);
}

pub fn confirm_rollback(area: Rect, theme: &Theme, buf: &mut Buffer) {
    let lines = vec![
        Line::styled(
            "Roll back this hunk?",
            Style::default().fg(color(theme.editor_fg)),
        ),
        Line::styled(
            "[Enter] yes / [Esc] no",
            Style::default().fg(color(theme.line_number)),
        ),
    ];
    popup(CONFIRM_WIDTH, lines, area, theme, buf);
}
