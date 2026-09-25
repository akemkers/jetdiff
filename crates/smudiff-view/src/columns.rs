//! Kolonner i en linje. Modellen bruker UTF-8 bytes, visningen trenger celler for bredde.
//! Tab går til neste tabstopp med bredde 4, som IntelliJ sin default `TAB_SIZE`.

pub const TAB_SIZE: usize = 4;

/// Tegn som hører til forrige tegn og ikke tar egen plass.
fn is_combining(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F | 0x200D | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF
    )
}

/// Grov East Asian Width uten tabell-crate. Brede tegn tar to celler, som i vim.
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

/// Bredde i celler etter tab-utvidelse.
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
    /// Per tab: byte-posisjon i originalen og ekstra bytes lagt til etter den.
    /// Tom når linja ikke har tabs, så vanlige linjer mappes uten oppslag.
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

    /// Byte i originalen til byte i vist tekst. En kolonne på en tab peker på starten av den.
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
    fn uten_tabs_er_identitet() {
        let l = ExpandedLine::new("abc");
        assert_eq!(l.text, "abc");
        assert_eq!(l.display_index(2), 2);
        assert_eq!(l.display_index(3), 3);
    }

    #[test]
    fn tab_gaar_til_neste_stopp() {
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
    fn tab_rett_paa_stopp_blir_fire() {
        let l = ExpandedLine::new("abcd\te");
        assert_eq!(l.text, "abcd    e");
        assert_eq!(l.display_index(5), 8);
    }

    #[test]
    fn flerbyte_tegn_teller_som_en_kolonne() {
        // 'ø' er to bytes, men én kolonne
        let l = ExpandedLine::new("ø\tx");
        assert_eq!(l.text, "ø   x");
        assert_eq!(l.display_index(2), 2);
        assert_eq!(l.display_index(3), 5);
        assert_eq!(display_width("ø\tx"), 5);
    }

    #[test]
    fn bredde() {
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("\t\t"), 8);
        assert_eq!(display_width("a\tb"), 5);
        assert_eq!(display_width("日本"), 4);
        // "e" + kombinerende akutt aksent
        assert_eq!(display_width("e\u{301}x"), 2);
    }
}
