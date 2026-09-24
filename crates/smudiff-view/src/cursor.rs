//! Én cursor for hele diffen, på den siden som har fokus. Siden er master for sync scroll,
//! som `getCurrentSide` i `TwosideTextDiffViewer`.

use smudiff_core::side::Side;

use crate::columns;

/// `$` husker at cursoren skal til slutten av hver linje, som vim sin `MAXCOL`.
const WANT_END: usize = usize::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub side: Side,
    /// Logisk linje. På en kollapset fold er det første linje i folden.
    pub line: usize,
    /// UTF-8 byte i linja, alltid på starten av et tegn.
    pub col: usize,
    /// Cellen j/k prøver å holde, vim sin `curswant`.
    pub want: usize,
}

impl Cursor {
    pub fn new(side: Side, line: usize) -> Self {
        Self {
            side,
            line,
            col: 0,
            want: 0,
        }
    }

    /// Ny linje med samme ønskede celle, som vim sin `coladvance(curswant)`.
    pub fn with_line(self, line: usize, text: &str) -> Self {
        let col = if self.want == WANT_END {
            columns::last_col(text)
        } else {
            columns::col_for_cell(text, self.want)
        };
        Self { line, col, ..self }
    }

    /// Horisontal flytting setter ny ønsket celle.
    pub fn with_col(self, col: usize, text: &str) -> Self {
        let col = columns::clamp_col(text, col);
        Self {
            col,
            want: columns::cursor_cell(text, col),
            ..self
        }
    }

    pub fn to_end(self, text: &str) -> Self {
        Self {
            col: columns::last_col(text),
            want: WANT_END,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j_holder_onsket_celle_over_korte_linjer() {
        let c = Cursor::new(Side::Left, 0).with_col(6, "abcdefgh");
        assert_eq!(c.want, 6);
        let c = c.with_line(1, "ab");
        assert_eq!(c.col, 1);
        let c = c.with_line(2, "abcdefgh");
        assert_eq!(c.col, 6);
    }

    #[test]
    fn dollar_folger_linjeslutt() {
        let c = Cursor::new(Side::Left, 0).to_end("abc");
        assert_eq!(c.col, 2);
        assert_eq!(c.with_line(1, "abcdef").col, 5);
        assert_eq!(c.with_line(1, "").col, 0);
    }

    #[test]
    fn tab_og_flerbyte() {
        // På taben står cursoren i celle 3, så j lander på 'd' (celle 3) i neste linje
        let c = Cursor::new(Side::Left, 0).with_col(0, "\tx");
        assert_eq!(c.want, 3);
        assert_eq!(c.with_line(1, "abcd").col, 3);
        let c = Cursor::new(Side::Left, 0).with_col(4, "æøå");
        assert_eq!((c.col, c.want), (4, 2));
        assert_eq!(c.with_line(1, "日本語").col, 3);
    }
}
