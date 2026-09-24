//! Kolonner i en linje. Modellen bruker UTF-8 bytes, cursoren trenger visningsceller for j/k.
//! Tab går til neste tabstopp med bredde 4, som IntelliJ sin default `TAB_SIZE`.

use std::ops::Range;

pub const TAB_SIZE: usize = 4;

/// Tegn som hører til forrige tegn. Vim flytter over dem sammen med basistegnet.
pub fn is_combining(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F | 0x200D | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF
    )
}

/// Grov East Asian Width uten tabell-crate. Brede tegn tar to celler, som i vim.
pub fn char_cells(ch: char) -> usize {
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

/// Et sted cursoren kan stå: et basistegn med påfølgende kombinerende tegn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub byte: usize,
    pub cells: Range<usize>,
}

pub fn stops(line: &str) -> Vec<Stop> {
    let mut out: Vec<Stop> = Vec::new();
    let mut cell = 0;
    for (i, ch) in line.char_indices() {
        if is_combining(ch) && !out.is_empty() {
            continue;
        }
        let width = if ch == '\t' {
            TAB_SIZE - cell % TAB_SIZE
        } else {
            char_cells(ch).max(1)
        };
        out.push(Stop {
            byte: i,
            cells: cell..cell + width,
        });
        cell += width;
    }
    out
}

/// Bredde i celler etter tab-utvidelse.
pub fn display_width(line: &str) -> usize {
    stops(line).last().map_or(0, |s| s.cells.end)
}

/// Stoppet som dekker byte-kolonnen. Normal mode står aldri etter siste tegn.
fn stop_index(stops: &[Stop], col: usize) -> Option<usize> {
    let i = stops.partition_point(|s| s.byte <= col);
    i.checked_sub(1).or((!stops.is_empty()).then_some(0))
}

/// Gyldig cursor-kolonne: starten på et tegn, og 0 på tom linje.
pub fn clamp_col(line: &str, col: usize) -> usize {
    let s = stops(line);
    stop_index(&s, col).map_or(0, |i| s[i].byte)
}

/// Neste stopp etter kolonnen, eller linjeslutt. Gir bredden på blokk-cursoren.
pub fn next_col(line: &str, col: usize) -> usize {
    let s = stops(line);
    match stop_index(&s, col) {
        Some(i) => s.get(i + 1).map_or(line.len(), |n| n.byte),
        None => 0,
    }
}

/// Vim sin `w_virtcol` i normal mode: på en tab står cursoren i siste celle.
pub fn cursor_cell(line: &str, col: usize) -> usize {
    let s = stops(line);
    let Some(i) = stop_index(&s, col) else {
        return 0;
    };
    if line[s[i].byte..].starts_with('\t') {
        s[i].cells.end - 1
    } else {
        s[i].cells.start
    }
}

/// Cellene tegnet dekker, for x-posisjon og bredde på blokk-cursoren. Tom linje gir én celle.
pub fn cell_range(line: &str, col: usize) -> Range<usize> {
    let s = stops(line);
    stop_index(&s, col).map_or(0..1, |i| s[i].cells.clone())
}

/// `coladvance`: tegnet som dekker cellen, ellers siste tegn.
pub fn col_for_cell(line: &str, want: usize) -> usize {
    let s = stops(line);
    s.iter()
        .find(|s| s.cells.contains(&want))
        .or(s.last())
        .map_or(0, |s| s.byte)
}

/// Flytt `delta` tegn innen linja, uten å bryte til neste som vim sin `whichwrap`.
pub fn move_col(line: &str, col: usize, delta: isize) -> usize {
    let s = stops(line);
    let Some(i) = stop_index(&s, col) else {
        return 0;
    };
    let j = i.saturating_add_signed(delta).min(s.len() - 1);
    s[j].byte
}

pub fn last_col(line: &str) -> usize {
    stops(line).last().map_or(0, |s| s.byte)
}

/// Tegnindeks for kolonnen. IntelliJ overfører kolonnen i tegn mellom sidene (`LineCol`).
pub fn char_index(line: &str, col: usize) -> usize {
    line[..col.min(line.len())].chars().count()
}

/// `LineCol.toOffset`: kolonnen klemmes til linjeslutt, så til siste tegn.
pub fn col_for_char_index(line: &str, index: usize) -> usize {
    let col = line
        .char_indices()
        .nth(index)
        .map_or(line.len(), |(i, _)| i);
    clamp_col(line, col)
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

    /// Motsatt av `display_index`. Alt inne i en utvidet tab gir taben.
    pub fn col_for_display_index(&self, index: usize) -> usize {
        let mut extra_before = 0;
        for &(pos, extra) in &self.tabs {
            if index < pos + extra_before {
                break;
            }
            if index <= pos + extra {
                return pos;
            }
            extra_before = extra;
        }
        index - extra_before
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
    }

    #[test]
    fn vist_indeks_tilbake_til_kolonne() {
        let l = ExpandedLine::new("\tab\tc");
        let back: Vec<_> = (0..10).map(|i| l.col_for_display_index(i)).collect();
        assert_eq!(back, [0, 0, 0, 0, 1, 2, 3, 3, 4, 5]);
        for col in 0..=5 {
            assert_eq!(l.col_for_display_index(l.display_index(col)), col);
        }
        assert_eq!(ExpandedLine::new("abc").col_for_display_index(2), 2);
    }

    #[test]
    fn cursor_pa_tab_star_i_siste_celle() {
        assert_eq!(cursor_cell("\tx", 0), 3);
        assert_eq!(cursor_cell("\tx", 1), 4);
        assert_eq!(cell_range("\tx", 0), 0..4);
        assert_eq!(cell_range("", 0), 0..1);
        // j fra celle 3 havner på taben i neste linje, og fra 4 på x
        assert_eq!(col_for_cell("ab\tx", 3), 2);
        assert_eq!(col_for_cell("ab\tx", 4), 3);
    }

    #[test]
    fn kolonner_ligger_pa_tegngrenser() {
        let line = "æøå🚀x";
        assert_eq!(clamp_col(line, 1), 0);
        assert_eq!(clamp_col(line, 7), 6);
        assert_eq!(clamp_col(line, 100), 10);
        assert_eq!(move_col(line, 0, 3), 6);
        assert_eq!(move_col(line, 6, 1), 10);
        assert_eq!(move_col(line, 10, 5), 10);
        assert_eq!(move_col(line, 10, -9), 0);
        assert_eq!(last_col(line), 10);
        assert_eq!(next_col(line, 6), 10);
        assert_eq!(next_col(line, 10), 11);
        assert_eq!(clamp_col("", 5), 0);
        assert_eq!(next_col("", 0), 0);
    }

    #[test]
    fn kombinerende_tegn_folger_basistegnet() {
        // "e" + kombinerende akutt aksent, så "x"
        let line = "e\u{301}x";
        assert_eq!(stops(line).len(), 2);
        assert_eq!(move_col(line, 0, 1), 3);
        assert_eq!(clamp_col(line, 1), 0);
        assert_eq!(display_width(line), 2);
    }

    #[test]
    fn brede_tegn_tar_to_celler() {
        let line = "日x";
        assert_eq!(cursor_cell(line, 3), 2);
        assert_eq!(col_for_cell(line, 1), 0);
        assert_eq!(col_for_cell(line, 2), 3);
        assert_eq!(col_for_cell(line, 99), 3);
    }

    #[test]
    fn tegnindeks_mellom_sider() {
        assert_eq!(char_index("æøå", 4), 2);
        assert_eq!(col_for_char_index("abc", 2), 2);
        // Klemmes til siste tegn
        assert_eq!(col_for_char_index("ab", 7), 1);
        assert_eq!(col_for_char_index("æø", 1), 2);
        assert_eq!(col_for_char_index("", 3), 0);
    }
}
