//! Farger fra IntelliJ sine default-temaer i New UI: "Islands Dark" og "Islands Light"
//! (`LafManagerImpl.DefaultNewUiThemeStrategy`). Dark bruker editor-skjemaet
//! `IslandSchemeDark.xml` med Darcula som forelder. Light bruker `expUI_lightScheme.xml` med
//! Default som forelder. I tillegg Catppuccin Mocha.

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
    /// Markert linje. Gjennomsiktig og sterkere enn `CARET_ROW_COLOR`, siden den er
    /// eneste markering og skal synes over diff-bakgrunnene.
    pub row_highlight: Rgba,
    /// Linjenummeret til markert linje. Gul, siden ingen diff-farge er gul.
    pub gutter_highlight: Rgba,
    pub indent_guide: Rgba,
    /// `DIFF_SEPARATOR_WAVE`, streken over foldede områder.
    pub separator_wave: Rgba,
    /// Grå tommel med alfa, som macOS-scrollbaren i IntelliJ.
    pub scrollbar_thumb: Rgba,
    /// `Tree.selectionBackground`.
    pub tree_selection: Rgba,
    /// Linja og prikkene i commit-lista.
    pub commit_graph: Rgba,
    /// Beskjed over tastelinja. Advarsler er gule.
    pub message_bg: Rgba,
    pub warning_bg: Rgba,
    pub message_fg: Rgba,
    /// `FileStatus` ADDED, DELETED og MODIFIED. Rename bruker MODIFIED.
    pub file_added: Rgba,
    pub file_deleted: Rgba,
    pub file_modified: Rgba,
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
            row_highlight: [0xFF, 0xFF, 0xFF, 0x12],
            gutter_highlight: rgb(0x8A6D2F),
            indent_guide: rgb(0x323438),
            separator_wave: rgb(0x555555),
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            tree_selection: rgb(0x2E436E),
            commit_graph: rgb(0x56A8F5),
            message_bg: rgb(0x73BD79),
            warning_bg: rgb(0xD5B778),
            message_fg: bg,
            file_added: rgb(0x73BD79),
            file_deleted: rgb(0x6F737A),
            file_modified: rgb(0x70AEFF),
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
                Token::FunctionCall | Token::Type | Token::Punctuation | Token::Operator => {
                    style(0xBCBEC4)
                }
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
            row_highlight: [0x2E, 0x6B, 0xE6, 0x1A],
            gutter_highlight: rgb(0xF5D98B),
            indent_guide: rgb(0xEBECF0),
            separator_wave: rgb(0xD0D0D0),
            scrollbar_thumb: [0x00, 0x00, 0x00, 0x33],
            tree_selection: rgb(0xD5E1FF),
            commit_graph: rgb(0x3574F0),
            message_bg: rgb(0xBEE6BE),
            warning_bg: rgb(0xF5D98B),
            message_fg: rgb(0x080808),
            file_added: rgb(0x067D17),
            file_deleted: rgb(0x6C707E),
            file_modified: rgb(0x0033B3),
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
                Token::FunctionCall | Token::Type | Token::Punctuation | Token::Operator => {
                    style(0x080808)
                }
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

    /// Catppuccin Mocha (no italics) fra `catppuccin/jetbrains` (`editor.tera`,
    /// `ui.theme.tera`). Blandede farger er regnet ut på forhånd, som
    /// `farge | mix(color=base, amount=a)`.
    pub fn mocha() -> Self {
        let base = rgb(0x1E1E2E);
        let text = 0xCDD6F4;
        Self {
            editor_bg: base,
            editor_fg: rgb(text),
            gutter_bg: base,
            divider_bg: base,
            line_number: rgb(0x6C7086),
            line_number_caret_row: rgb(0xB4BEFE),
            row_highlight: [0xFF, 0xFF, 0xFF, 0x12],
            // Yellow med 0.5
            gutter_highlight: rgb(0x8C806E),
            indent_guide: rgb(0x313244),
            // Arvet fra Darcula
            separator_wave: rgb(0x555555),
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            tree_selection: rgb(0x353748),
            commit_graph: rgb(0x94E2D5),
            message_bg: rgb(0xA6E3A1),
            warning_bg: rgb(0xF9E2AF),
            message_fg: base,
            file_added: rgb(0xA6E3A1),
            file_deleted: rgb(0x585B70),
            file_modified: rgb(0x89B4FA),
            // Green, red og blue med 0.25 for bakgrunn og 0.2 for stripe
            inserted: DiffColors::new(rgb(0x404F4B), rgb(0x394545), base),
            deleted: DiffColors::new(rgb(0x53394C), rgb(0x493446), base),
            modified: DiffColors::new(rgb(0x394461), rgb(0x333C57), base),
            syntax: Token::ALL.map(|t| match t {
                Token::Keyword => style(0xCBA6F7),
                Token::String => style(0xA6E3A1),
                Token::Escape => style(0xF5C2E7),
                Token::Number | Token::Constant => style(0xFAB387),
                Token::Comment | Token::DocComment => style(0x9399B2),
                Token::Function | Token::FunctionCall => style(0x89B4FA),
                Token::Type => style(0xF9E2AF),
                Token::Property => style(text),
                Token::Metadata => style(0xF9E2AF),
                Token::Tag => style(0x94E2D5),
                Token::Attribute => style(0xF9E2AF),
                Token::Heading => bold(0xF38BA8),
                Token::Link => style(0xB4BEFE),
                Token::Strong => bold(0xF38BA8),
                Token::Emphasis => style(0xF38BA8),
                Token::Punctuation => style(0x9399B2),
                Token::Operator => style(0x89DCEB),
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
