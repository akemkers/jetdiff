//! One line as cells.

use std::ops::Range;

use jetdiff_viewmodel::columns::ExpandedLine;
use jetdiff_viewmodel::model::SyntaxSpan;
use jetdiff_viewmodel::{DiffType, InlineHighlight, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::color::color;

pub struct LineText<'a> {
    pub text: &'a str,
    pub syntax: &'a [SyntaxSpan],
    pub inline: &'a [InlineHighlight],
    /// Unified colors the words by the row, not by the word.
    pub inline_type: Option<DiffType>,
}

/// `area` is one row with the background from before.
pub fn paint_line(line: &LineText, scroll_x: usize, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let expanded = ExpandedLine::new(line.text);
    let display =
        |cols: &Range<usize>| expanded.display_index(cols.start)..expanded.display_index(cols.end);
    let syntax: Vec<_> = line
        .syntax
        .iter()
        .map(|s| (display(&s.cols), s.token))
        .collect();
    let inline: Vec<_> = line
        .inline
        .iter()
        .map(|h| {
            let bg = color(
                theme
                    .diff(line.inline_type.unwrap_or(h.diff_type))
                    .background,
            );
            let end = if h.past_line_end {
                usize::MAX
            } else {
                display(&h.cols).end
            };
            (display(&h.cols).start..end, bg)
        })
        .collect();

    let width = area.width as usize;
    let mut col = 0;
    let mut text_style = Style::default().fg(color(theme.editor_fg));
    let (mut next_syntax, mut next_inline) = (0, 0);
    for (at, ch) in expanded.text.char_indices() {
        let cells = ch.width().unwrap_or(0);
        if col >= scroll_x + width {
            break;
        }
        if col + cells > scroll_x && cells > 0 {
            let x = area.x + col.saturating_sub(scroll_x) as u16;
            let cell = &mut buf[(x, area.y)];
            // A wide character that starts before the scroll is shown as spaces
            let shown = if col < scroll_x || col + cells > scroll_x + width {
                ' '
            } else {
                ch
            };
            cell.set_char(shown);
            // A space keeps the style in front of it, so the terminal is spared a color change
            if !shown.is_whitespace() {
                text_style = Style::default().fg(color(theme.editor_fg));
                if let Some(token) = covering(&syntax, &mut next_syntax, at) {
                    let s = theme.syntax(token);
                    text_style = text_style.fg(color(s.color));
                    if s.bold {
                        text_style = text_style.add_modifier(Modifier::BOLD);
                    }
                    if s.italic {
                        text_style = text_style.add_modifier(Modifier::ITALIC);
                    }
                }
            }
            let mut style = text_style;
            if let Some(bg) = covering(&inline, &mut next_inline, at) {
                style = style.bg(bg);
            }
            cell.set_style(style);
        }
        col += cells;
    }
    let end = expanded.text.len();
    if let Some((_, bg)) = inline
        .iter()
        .find(|(r, _)| r.end == usize::MAX && r.start <= end)
    {
        let from = col.saturating_sub(scroll_x).min(width) as u16;
        fill(
            Rect {
                x: area.x + from,
                width: area.width - from,
                ..area
            },
            *bg,
            buf,
        );
    }
}

/// The span that covers `at`. The spans are sorted, and `at` increases, so `next` only goes forward.
fn covering<T: Copy>(spans: &[(Range<usize>, T)], next: &mut usize, at: usize) -> Option<T> {
    while spans.get(*next).is_some_and(|(r, _)| r.end <= at) {
        *next += 1;
    }
    spans
        .get(*next)
        .filter(|(r, _)| r.start <= at)
        .map(|&(_, value)| value)
}

pub fn fill(area: Rect, bg: Color, buf: &mut Buffer) {
    buf.set_style(area, Style::default().bg(bg));
}

pub fn tint(area: Rect, tint: jetdiff_viewmodel::Rgba, buf: &mut Buffer) {
    for x in area.left()..area.right() {
        for y in area.top()..area.bottom() {
            let cell = &mut buf[(x, y)];
            cell.set_bg(crate::color::over(tint, cell.bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use jetdiff_viewmodel::Token;

    use super::*;

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_owned())
            .collect()
    }

    fn paint(text: &str, scroll_x: usize, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let line = LineText {
            text,
            syntax: &[],
            inline: &[],
            inline_type: None,
        };
        paint_line(&line, scroll_x, area, &Theme::mocha(), &mut buf);
        row(&buf, 0)
    }

    #[test]
    fn tabs_and_scroll() {
        assert_eq!(paint("a\tb", 0, 6), "a   b ");
        assert_eq!(paint("abcdef", 2, 3), "cde");
        assert_eq!(paint("ab", 5, 3), "   ");
    }

    #[test]
    fn space_keeps_the_color_in_front() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = Buffer::empty(area);
        let theme = Theme::mocha();
        let line = LineText {
            text: "fn x",
            syntax: &[SyntaxSpan {
                cols: 0..2,
                token: Token::Keyword,
            }],
            inline: &[],
            inline_type: None,
        };
        paint_line(&line, 0, area, &theme, &mut buf);
        let keyword = color(theme.syntax(Token::Keyword).color);
        assert_eq!(buf[(2, 0)].fg, keyword);
        assert_eq!(buf[(3, 0)].fg, color(theme.editor_fg));
    }

    #[test]
    fn wide_characters_are_cut_to_spaces() {
        assert_eq!(paint("日本", 1, 3), " 本 ");
    }
}
