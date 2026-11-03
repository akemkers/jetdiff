//! Prose from the model, drawn with its marks: `code` in backticks as inline code, figures in bold,
//! and for a point, its lead-in up to the first colon or full stop in bold.

use std::ops::Range;

use gpui::{Font, FontWeight, Hsla, StyledText, TextRun, font};

use super::palette::{CODE_BACKGROUND, color};
use crate::text::FAMILY;

/// The system's own sans, so prose doesn't read like code.
pub(super) const SANS: &str = ".SystemUIFont";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Plain,
    Bold,
    Code,
}

/// `text` without its backticks, and the marks on it, in order and covering all of it.
pub(super) fn marks(text: &str, lead_in: bool) -> (String, Vec<(Mark, Range<usize>)>) {
    let (text, code) = without_backticks(text);
    let mut marks = vec![Mark::Plain; text.len()];
    let mut set = |range: Range<usize>, mark: Mark| marks[range].fill(mark);
    let in_code = |at: usize| code.iter().any(|range| range.contains(&at));
    if lead_in && let Some(end) = lead_in_end(&text, &in_code) {
        set(0..end, Mark::Bold);
    }
    for word in figures(&text, &in_code) {
        set(word, Mark::Bold);
    }
    for range in &code {
        set(range.clone(), Mark::Code);
    }
    (text, runs(&marks))
}

/// A backtick without its pair is kept as text.
fn without_backticks(text: &str) -> (String, Vec<Range<usize>>) {
    let parts: Vec<&str> = text.split('`').collect();
    let paired = parts.len() % 2 == 1;
    let mut plain = String::with_capacity(text.len());
    let mut code = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let is_code = i % 2 == 1 && (paired || i + 1 < parts.len());
        if i % 2 == 1 && !is_code {
            plain.push('`');
        }
        let start = plain.len();
        plain.push_str(part);
        if is_code {
            code.push(start..plain.len());
        }
    }
    (plain, code)
}

/// After the first colon, semicolon or full stop outside code, when more text follows.
fn lead_in_end(text: &str, in_code: &impl Fn(usize) -> bool) -> Option<usize> {
    let mut chars = text.char_indices().peekable();
    while let Some((at, char)) = chars.next() {
        let ends = match char {
            ':' | ';' => true,
            '.' => chars.peek().is_some_and(|(_, next)| next.is_whitespace()),
            _ => false,
        };
        if ends && !in_code(at) {
            let end = at + char.len_utf8();
            return (end < text.trim_end().len()).then_some(end);
        }
    }
    None
}

/// Words with a digit, like `640k` or `200`, without the punctuation after them.
fn figures(text: &str, in_code: &impl Fn(usize) -> bool) -> Vec<Range<usize>> {
    let mut words = Vec::new();
    let mut start = None;
    for (at, char) in text.char_indices().chain([(text.len(), ' ')]) {
        match (start, char.is_whitespace()) {
            (None, false) => start = Some(at),
            (Some(word_start), true) => {
                let word = text[word_start..at].trim_end_matches([',', '.', ';', ':', ')']);
                let end = word_start + word.len();
                if word.chars().any(|char| char.is_ascii_digit()) && !in_code(word_start) {
                    words.push(word_start..end);
                }
                start = None;
            }
            _ => {}
        }
    }
    words
}

fn runs(marks: &[Mark]) -> Vec<(Mark, Range<usize>)> {
    let mut runs: Vec<(Mark, Range<usize>)> = Vec::new();
    for (at, &mark) in marks.iter().enumerate() {
        match runs.last_mut() {
            Some((last, range)) if *last == mark => range.end = at + 1,
            _ => runs.push((mark, at..at + 1)),
        }
    }
    runs
}

/// The colors of plain and bold text. Code has its own.
pub(super) struct ProseColors {
    pub text: Hsla,
    pub bold: Hsla,
}

pub(super) fn prose(text: &str, lead_in: bool, colors: &ProseColors) -> StyledText {
    let (text, marks) = marks(text, lead_in);
    let sans = font(SANS);
    let runs = marks
        .into_iter()
        .map(|(mark, range)| {
            let (font, color, background_color) = match mark {
                Mark::Plain => (sans.clone(), colors.text, None),
                Mark::Bold => (bold(&sans), colors.bold, None),
                Mark::Code => (font(FAMILY), colors.bold, Some(color(CODE_BACKGROUND))),
            };
            TextRun {
                len: range.len(),
                font,
                color,
                background_color,
                underline: None,
                strikethrough: None,
            }
        })
        .collect();
    StyledText::new(text).with_runs(runs)
}

fn bold(font: &Font) -> Font {
    Font {
        weight: FontWeight::SEMIBOLD,
        ..font.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of each mark, like `**bold**` and `` `code` ``.
    fn shown(text: &str, lead_in: bool) -> String {
        let (text, marks) = marks(text, lead_in);
        marks
            .into_iter()
            .map(|(mark, range)| match mark {
                Mark::Plain => text[range].to_owned(),
                Mark::Bold => format!("**{}**", &text[range]),
                Mark::Code => format!("`{}`", &text[range]),
            })
            .collect()
    }

    #[test]
    fn code_in_backticks() {
        assert_eq!(
            shown("Press `w` to open `RepoView`", false),
            "Press `w` to open `RepoView`"
        );
        let (text, _) = marks("Press `w` to open", false);
        assert_eq!(text, "Press w to open");
        // Without its pair the backtick is text
        assert_eq!(shown("a ` b", false), "a ` b");
    }

    #[test]
    fn figures_in_bold() {
        assert_eq!(
            shown("Over 640k characters, up to 200.", false),
            "Over **640k** characters, up to **200**."
        );
        assert_eq!(shown("Keep `h3` as it is", false), "Keep `h3` as it is");
    }

    #[test]
    fn the_lead_in_of_a_point() {
        assert_eq!(
            shown(
                "The digest is central: every hunk has an alias like `h3`.",
                true
            ),
            "**The digest is central:** every hunk has an alias like `h3`."
        );
        assert_eq!(
            shown("No API client. It runs `claude`.", true),
            "**No API client.** It runs `claude`."
        );
        // A point that is only a lead-in, or a full stop inside a name, is not bold
        assert_eq!(shown("Just one sentence.", true), "Just one sentence.");
        assert_eq!(
            shown("Uses `Job::run.` and more", true),
            "Uses `Job::run.` and more"
        );
    }
}
