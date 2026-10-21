//! Columns in a line. The model uses UTF-8 bytes; the view needs cells for width.
//! Tab goes to the next tab stop with width 4, like IntelliJ's default `TAB_SIZE`.

pub const TAB_SIZE: usize = 4;

/// Characters that belong to the previous character and take no space of their own.
fn is_combining(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F | 0x200D | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF
    )
}

/// Rough East Asian Width without a table crate. Wide characters take two cells, as in vim.
fn char_cells(ch: char) -> usize {
    if is_combining(ch) {
        return 0;
    }
    let wide = matches!(
        ch as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    );
    if wide { 2 } else { 1 }
}

/// Width in cells after tab expansion.
pub fn display_width(line: &str) -> usize {
    line.chars().fold(0, |cell, ch| {
        cell + if ch == '\t' {
            TAB_SIZE - cell % TAB_SIZE
        } else {
            char_cells(ch)
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedLine {
    pub text: String,
    /// Per tab: byte position in the original and extra bytes added after it.
    /// Empty when the line has no tabs, so ordinary lines are mapped without a lookup.
    tabs: Vec<(usize, usize)>,
}

impl ExpandedLine {
    pub fn new(line: &str) -> Self {
        if !line.contains('\t') {
            return Self {
                text: line.to_owned(),
                tabs: Vec::new(),
            };
        }
        let mut text = String::with_capacity(line.len() + 8);
        let mut tabs = Vec::new();
        let mut col = 0;
        let mut extra = 0;
        for (i, ch) in line.char_indices() {
            if ch == '\t' {
                let width = TAB_SIZE - col % TAB_SIZE;
                text.extend(std::iter::repeat_n(' ', width));
                col += width;
                extra += width - 1;
                tabs.push((i, extra));
            } else {
                text.push(ch);
                col += 1;
            }
        }
        Self { text, tabs }
    }

    /// Byte in the original to byte in the shown text. A column on a tab points at its start.
    pub fn display_index(&self, col: usize) -> usize {
        let before = self.tabs.partition_point(|&(pos, _)| pos < col);
        match before {
            0 => col,
            n => col + self.tabs[n - 1].1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_tabs_is_identity() {
        let l = ExpandedLine::new("abc");
        assert_eq!(l.text, "abc");
        assert_eq!(l.display_index(2), 2);
        assert_eq!(l.display_index(3), 3);
    }

    #[test]
    fn tab_goes_to_next_stop() {
        let l = ExpandedLine::new("\tab\tc");
        assert_eq!(l.text, "    ab  c");
        assert_eq!(l.display_index(0), 0);
        assert_eq!(l.display_index(1), 4);
        assert_eq!(l.display_index(2), 5);
        assert_eq!(l.display_index(3), 6);
        assert_eq!(l.display_index(4), 8);
        assert_eq!(l.display_index(5), 9);
    }

    #[test]
    fn tab_right_on_stop_becomes_four() {
        let l = ExpandedLine::new("abcd\te");
        assert_eq!(l.text, "abcd    e");
        assert_eq!(l.display_index(5), 8);
    }

    #[test]
    fn multibyte_char_counts_as_one_column() {
        // 'ø' is two bytes, but one column
        let l = ExpandedLine::new("ø\tx");
        assert_eq!(l.text, "ø   x");
        assert_eq!(l.display_index(2), 2);
        assert_eq!(l.display_index(3), 5);
        assert_eq!(display_width("ø\tx"), 5);
    }

    #[test]
    fn width() {
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("\t\t"), 8);
        assert_eq!(display_width("a\tb"), 5);
        assert_eq!(display_width("日本"), 4);
        // "e" + combining acute accent
        assert_eq!(display_width("e\u{301}x"), 2);
    }
}
