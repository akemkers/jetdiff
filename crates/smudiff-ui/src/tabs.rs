//! Tab-utvidelse for visning. Tab går til neste tabstopp med bredde 4, som IntelliJ sin
//! default `TAB_SIZE`. Kolonner fra modellen er UTF-8 bytes i originallinja og må mappes
//! til bytes i den viste teksten før x hentes fra den shapede linja.

pub const TAB_SIZE: usize = 4;

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

/// Bredde i tegn etter utvidelse, for å anslå maks horisontal scroll uten å shape alt.
pub fn display_width(line: &str) -> usize {
    let mut col = 0;
    for ch in line.chars() {
        col += if ch == '\t' {
            TAB_SIZE - col % TAB_SIZE
        } else {
            1
        };
    }
    col
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
    }
}
