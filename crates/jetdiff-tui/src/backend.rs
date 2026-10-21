//! A `CrosstermBackend` that only sends the color that changed. ratatui sends the foreground and
//! background each time one of them changes, and the background is then most of what is sent during
//! scroll.

use std::io::{self, Write};

use ratatui::backend::{Backend, ClearType, CrosstermBackend, IntoCrossterm, WindowSize};
use ratatui::buffer::Cell;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::queue;
use ratatui::crossterm::style::{
    Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};

/// Only the ones jetdiff uses.
const ATTRIBUTES: [(Modifier, Attribute); 2] = [
    (Modifier::BOLD, Attribute::Bold),
    (Modifier::ITALIC, Attribute::Italic),
];

pub struct TermBackend<W: Write>(CrosstermBackend<W>);

impl<W: Write> TermBackend<W> {
    pub fn new(writer: W) -> Self {
        Self(CrosstermBackend::new(writer))
    }
}

impl<W: Write> Write for TermBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.0)
    }
}

impl<W: Write> Backend for TermBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let writer = self.0.writer_mut();
        let (mut fg, mut bg, mut modifier) = (Color::Reset, Color::Reset, Modifier::empty());
        let mut last = None;
        for (x, y, cell) in content {
            if last != Some((x.wrapping_sub(1), y)) {
                queue!(writer, MoveTo(x, y))?;
            }
            last = Some((x, y));
            if cell.modifier != modifier {
                // Reset clears the colors too
                queue!(writer, SetAttribute(Attribute::Reset))?;
                (fg, bg) = (Color::Reset, Color::Reset);
                for (flag, attribute) in ATTRIBUTES {
                    if cell.modifier.contains(flag) {
                        queue!(writer, SetAttribute(attribute))?;
                    }
                }
                modifier = cell.modifier;
            }
            if cell.fg != fg {
                queue!(writer, SetForegroundColor(cell.fg.into_crossterm()))?;
                fg = cell.fg;
            }
            if cell.bg != bg {
                queue!(writer, SetBackgroundColor(cell.bg.into_crossterm()))?;
                bg = cell.bg;
            }
            queue!(writer, Print(cell.symbol()))?;
        }
        queue!(writer, SetAttribute(Attribute::Reset))
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.0.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.0.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.0.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.0.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.0.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.0.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.0.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.0.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.0.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;

    fn draw(cells: &[(char, Style)]) -> String {
        let cells: Vec<Cell> = cells
            .iter()
            .map(|&(ch, style)| {
                let mut cell = Cell::default();
                cell.set_char(ch).set_style(style);
                cell
            })
            .collect();
        let mut backend = TermBackend::new(Vec::new());
        let content = cells.iter().enumerate().map(|(x, c)| (x as u16, 0, c));
        backend.draw(content).unwrap();
        String::from_utf8(backend.0.writer().clone()).unwrap()
    }

    #[test]
    fn sends_only_the_changed_color() {
        let bg = Color::Rgb(1, 2, 3);
        let a = Style::default().fg(Color::Rgb(9, 9, 9)).bg(bg);
        let b = a.fg(Color::Rgb(8, 8, 8));
        assert_eq!(
            draw(&[('a', a), ('b', b)]),
            "\x1b[1;1H\x1b[38;2;9;9;9m\x1b[48;2;1;2;3ma\x1b[38;2;8;8;8mb\x1b[0m"
        );
    }

    #[test]
    fn style_reset_sends_the_colors_again() {
        let a = Style::default().fg(Color::Rgb(9, 9, 9));
        let bold = a.add_modifier(Modifier::BOLD);
        assert_eq!(
            draw(&[('a', bold), ('b', a)]),
            "\x1b[1;1H\x1b[0m\x1b[1m\x1b[38;2;9;9;9ma\x1b[0m\x1b[38;2;9;9;9mb\x1b[0m"
        );
    }
}
