//! Farger fra IntelliJ sine default-temaer i New UI: "Islands Dark" og "Islands Light"
//! (`LafManagerImpl.DefaultNewUiThemeStrategy`). Dark bruker editor-skjemaet
//! `IslandSchemeDark.xml` med Darcula som forelder. Light bruker `expUI_lightScheme.xml` med
//! Default som forelder.

use smudiff_syntax::Token;

/// RGBA, alfa 255 er ugjennomsiktig.
pub type Rgba = [u8; 4];

const fn rgb(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColors {
    /// Linjebakgrunn og ord-highlight, og fyllet i divideren.
    pub background: Rgba,
    /// `ERROR_STRIPE_COLOR` i scrollbaren.
    pub stripe: Rgba,
    /// Bakgrunn for linjer med ord-highlight (`TextDiffTypeFactory.getIgnoredBackgroundColor`).
    pub ignored: Rgba,
}

impl DiffColors {
    fn new(background: Rgba, stripe: Rgba, editor_bg: Rgba) -> Self {
        Self {
            background,
            stripe,
            ignored: ignored_color(background, editor_bg),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxStyle {
    pub color: Rgba,
    pub bold: bool,
    pub italic: bool,
}

const fn style(hex: u32) -> SyntaxStyle {
    SyntaxStyle {
        color: rgb(hex),
        bold: false,
        italic: false,
    }
}

const fn italic(hex: u32) -> SyntaxStyle {
    SyntaxStyle {
        italic: true,
        ..style(hex)
    }
}

const fn bold(hex: u32) -> SyntaxStyle {
    SyntaxStyle {
        bold: true,
        ..style(hex)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub editor_bg: Rgba,
    pub editor_fg: Rgba,
    /// `EDITOR_GUTTER_BACKGROUND` er tom i begge temaene, så gutteren arver editor-bakgrunnen.
    pub gutter_bg: Rgba,
    /// `DiffDrawUtil.getDividerColor`: divideren males med gutter-bakgrunnen.
    pub divider_bg: Rgba,
    pub line_number: Rgba,
    pub line_number_caret_row: Rgba,
    /// `CARET_COLOR`.
    pub caret: Rgba,
    /// `CARET_ROW_COLOR`.
    pub caret_row: Rgba,
    pub indent_guide: Rgba,
    /// `DIFF_SEPARATOR_WAVE`, streken over foldede områder.
    pub separator_wave: Rgba,
    /// Grå tommel med alfa, som macOS-scrollbaren i IntelliJ.
    pub scrollbar_thumb: Rgba,
    pub inserted: DiffColors,
    pub deleted: DiffColors,
    pub modified: DiffColors,
    /// Indeksert med `Token as usize`.
    syntax: [SyntaxStyle; Token::ALL.len()],
}

impl Theme {
    /// Islands Dark. Ingen `DIFF_*`-nøkler i skjemaet, så diff-fargene kommer fra Darcula.
    pub fn dark() -> Self {
        let bg = rgb(0x191A1C);
        Self {
            editor_bg: bg,
            editor_fg: rgb(0xBCBEC4),
            gutter_bg: bg,
            divider_bg: bg,
            line_number: rgb(0x4B5059),
            line_number_caret_row: rgb(0xA1A3AB),
            caret: rgb(0xCED0D6),
            caret_row: rgb(0x1F2024),
            indent_guide: rgb(0x323438),
            separator_wave: rgb(0x555555),
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            inserted: DiffColors::new(rgb(0x294436), rgb(0x447152), bg),
            deleted: DiffColors::new(rgb(0x484A4A), rgb(0x656E76), bg),
            modified: DiffColors::new(rgb(0x385570), rgb(0x43698D), bg),
            syntax: Token::ALL.map(|t| match t {
                Token::Keyword | Token::Escape => style(0xCF8E6D),
                Token::String => style(0x6AAB73),
                Token::Number => style(0x2AACB8),
                Token::Comment => style(0x7A7E85),
                Token::DocComment => italic(0x5F826B),
                Token::Function => style(0x56A8F5),
                Token::Constant => italic(0xC77DBB),
                Token::Property => style(0xC77DBB),
                Token::Metadata => style(0xB3AE60),
                Token::Tag => style(0xD5B778),
                Token::Attribute => style(0xBABABA),
                Token::Heading => bold(0xCF8E6D),
                Token::Link => style(0x548AF7),
                Token::Strong => bold(0xBCBEC4),
                Token::Emphasis => italic(0xBCBEC4),
            }),
        }
    }

    /// Islands Light. MODIFIED kommer fra Light, INSERTED og DELETED fra Default.
    pub fn light() -> Self {
        let bg = rgb(0xFFFFFF);
        Self {
            editor_bg: bg,
            editor_fg: rgb(0x080808),
            gutter_bg: bg,
            divider_bg: bg,
            line_number: rgb(0xAEB3C2),
            line_number_caret_row: rgb(0x767A8A),
            // Arvet fra Default-skjemaet, som har svart caret
            caret: rgb(0x000000),
            caret_row: rgb(0xF5F8FE),
            indent_guide: rgb(0xEBECF0),
            separator_wave: rgb(0xD0D0D0),
            scrollbar_thumb: [0x00, 0x00, 0x00, 0x33],
            inserted: DiffColors::new(rgb(0xBEE6BE), rgb(0xAADEAA), bg),
            deleted: DiffColors::new(rgb(0xD6D6D6), rgb(0xC8C8C8), bg),
            modified: DiffColors::new(rgb(0xC2D8F2), rgb(0xB6D2F2), bg),
            syntax: Token::ALL.map(|t| match t {
                Token::Keyword | Token::Tag => style(0x0033B3),
                Token::String => style(0x067D17),
                Token::Escape => style(0x0037A6),
                Token::Number => style(0x1750EB),
                Token::Comment | Token::DocComment => italic(0x8C8C8C),
                Token::Function => style(0x00627A),
                Token::Constant => italic(0x871094),
                Token::Property => style(0x871094),
                Token::Metadata => style(0x9E880D),
                Token::Attribute => style(0x174AD4),
                Token::Heading => bold(0x0033B3),
                Token::Link => style(0x006DCC),
                Token::Strong => bold(0x080808),
                Token::Emphasis => italic(0x080808),
            }),
        }
    }

    pub fn syntax(&self, token: Token) -> SyntaxStyle {
        self.syntax[token as usize]
    }
}

/// `ColorUtil.mix(background, editorBg, 0.6)`. Ingen av skjemaene setter FOREGROUND på
/// diff-nøklene, så ignored-fargen er alltid avledet.
fn ignored_color(background: Rgba, editor_bg: Rgba) -> Rgba {
    mix(background, editor_bg, 0.6)
}

/// `MixedColorProducer`: `c0 + (int) Math.round(ratio * (c1 - c0))` per kanal.
fn mix(c0: Rgba, c1: Rgba, ratio: f64) -> Rgba {
    let mut out = [0; 4];
    for i in 0..4 {
        let (a, b) = (c0[i] as f64, c1[i] as f64);
        // Java `Math.round` er floor(x + 0.5), også for negative tall
        out[i] = (a + (ratio * (b - a) + 0.5).floor()) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_i_dark() {
        let t = Theme::dark();
        assert_eq!(t.inserted.ignored, rgb(0x1F2B26));
        assert_eq!(t.deleted.ignored, rgb(0x2C2D2E));
        assert_eq!(t.modified.ignored, rgb(0x25323E));
    }

    #[test]
    fn ignored_i_light() {
        let t = Theme::light();
        assert_eq!(t.inserted.ignored, rgb(0xE5F5E5));
        assert_eq!(t.deleted.ignored, rgb(0xEFEFEF));
        assert_eq!(t.modified.ignored, rgb(0xE7EFFA));
    }

    #[test]
    fn mix_runder_som_java() {
        // 0.6 * -5 = -3.0, 0.6 * -1 = -0.6 gir -1, 0.5 * -1 = -0.5 gir 0 i Java
        assert_eq!(mix([10, 0, 0, 255], [5, 0, 0, 255], 0.6)[0], 7);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.6)[0], 0);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.5)[0], 1);
    }
}
