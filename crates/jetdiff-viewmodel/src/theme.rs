//! Colors from IntelliJ's default themes in New UI: "Islands Dark" and "Islands Light"
//! (`LafManagerImpl.DefaultNewUiThemeStrategy`). Dark uses the editor scheme
//! `IslandSchemeDark.xml` with Darcula as parent. Light uses `expUI_lightScheme.xml` with
//! Default as parent. Plus Catppuccin Mocha.

use jetdiff_syntax::Token;

use crate::model::DiffType;

/// RGBA, alpha 255 is opaque.
pub type Rgba = [u8; 4];

const fn rgb(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColors {
    /// Line background and inline highlight, and the fill in the divider.
    pub background: Rgba,
    /// `ERROR_STRIPE_COLOR` in the scrollbar.
    pub stripe: Rgba,
    /// Background for lines with inline highlight (`TextDiffTypeFactory.getIgnoredBackgroundColor`).
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
    /// Picks the `_dark` icons.
    pub dark: bool,
    /// Also the gutter and divider. `EDITOR_GUTTER_BACKGROUND` is empty in IntelliJ, so they inherit it.
    pub editor_bg: Rgba,
    pub editor_fg: Rgba,
    pub line_number: Rgba,
    pub line_number_caret_row: Rgba,
    /// The cursor's line. Transparent and stronger than `CARET_ROW_COLOR`, since it is
    /// the only highlight and must show over the diff backgrounds.
    pub row_highlight: Rgba,
    /// The line number of the cursor's line. Yellow, since no diff color is yellow.
    pub gutter_highlight: Rgba,
    pub indent_guide: Rgba,
    /// `DIFF_SEPARATOR_WAVE`, the stroke over folded areas.
    pub separator_wave: Rgba,
    /// Gray thumb with alpha, like the macOS scrollbar in IntelliJ.
    pub scrollbar_thumb: Rgba,
    /// `Tree.selectionBackground`.
    pub tree_selection: Rgba,
    /// `Tree.selectionInactiveBackground`, when the panel doesn't have focus.
    pub tree_selection_inactive: Rgba,
    /// The line and dots in the commit list.
    pub commit_graph: Rgba,
    /// Message above the keymap bar. Warnings are yellow.
    pub message_bg: Rgba,
    pub warning_bg: Rgba,
    pub message_fg: Rgba,
    /// `FileStatus` ADDED, DELETED and MODIFIED. Rename uses MODIFIED.
    pub file_added: Rgba,
    pub file_deleted: Rgba,
    pub file_modified: Rgba,
    pub inserted: DiffColors,
    pub deleted: DiffColors,
    pub modified: DiffColors,
    /// Indexed with `Token as usize`.
    syntax: [SyntaxStyle; Token::ALL.len()],
}

impl Theme {
    /// Islands Dark. No `DIFF_*` keys in the scheme, so the diff colors come from Darcula.
    pub fn dark() -> Self {
        let bg = rgb(0x191A1C);
        let fg = 0xBCBEC4;
        Self {
            dark: true,
            editor_bg: bg,
            editor_fg: rgb(fg),
            line_number: rgb(0x4B5059),
            line_number_caret_row: rgb(0xA1A3AB),
            row_highlight: [0xFF, 0xFF, 0xFF, 0x12],
            gutter_highlight: rgb(0x8A6D2F),
            indent_guide: rgb(0x323438),
            separator_wave: rgb(0x555555),
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            tree_selection: rgb(0x2E436E),
            tree_selection_inactive: rgb(0x43454A),
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
                Token::Keyword => style(0xCF8E6D),
                Token::String => style(0x6AAB73),
                Token::Escape => style(0xCF8E6D),
                Token::Number => style(0x2AACB8),
                Token::Comment => style(0x7A7E85),
                Token::DocComment => italic(0x5F826B),
                Token::Function => style(0x56A8F5),
                Token::FunctionCall | Token::Type => style(fg),
                Token::Constant => italic(0xC77DBB),
                Token::Property => style(0xC77DBB),
                Token::Metadata => style(0xB3AE60),
                Token::Tag => style(0xD5B778),
                Token::Attribute => style(0xBABABA),
                Token::Heading => bold(0xCF8E6D),
                Token::Link => style(0x548AF7),
                Token::Strong => bold(fg),
                Token::Emphasis => italic(fg),
                Token::Punctuation | Token::Operator => style(fg),
                // Islands Dark inherits Darcula, so the Rust plugin uses `RustDarcula.xml`
                Token::Struct => style(0x86985D),
                Token::Enum => italic(0x86985D),
                Token::EnumVariant => italic(0x6DA1AB),
                Token::Crate => style(0x8D91DC),
                Token::Macro => style(0x4EADE5),
                Token::SelfValue => style(0xD98EB3),
                Token::QOperator => bold(0xCC7832),
            }),
        }
    }

    /// Islands Light. MODIFIED comes from Light, INSERTED and DELETED from Default.
    pub fn light() -> Self {
        let bg = rgb(0xFFFFFF);
        let fg = 0x080808;
        Self {
            dark: false,
            editor_bg: bg,
            editor_fg: rgb(fg),
            line_number: rgb(0xAEB3C2),
            line_number_caret_row: rgb(0x767A8A),
            row_highlight: [0x2E, 0x6B, 0xE6, 0x1A],
            gutter_highlight: rgb(0xF5D98B),
            indent_guide: rgb(0xEBECF0),
            separator_wave: rgb(0xD0D0D0),
            scrollbar_thumb: [0x00, 0x00, 0x00, 0x33],
            tree_selection: rgb(0xD5E1FF),
            tree_selection_inactive: rgb(0xDFE1E5),
            commit_graph: rgb(0x3574F0),
            message_bg: rgb(0xBEE6BE),
            warning_bg: rgb(0xF5D98B),
            message_fg: rgb(fg),
            file_added: rgb(0x067D17),
            file_deleted: rgb(0x6C707E),
            file_modified: rgb(0x0033B3),
            inserted: DiffColors::new(rgb(0xBEE6BE), rgb(0xAADEAA), bg),
            deleted: DiffColors::new(rgb(0xD6D6D6), rgb(0xC8C8C8), bg),
            modified: DiffColors::new(rgb(0xC2D8F2), rgb(0xB6D2F2), bg),
            syntax: Token::ALL.map(|t| match t {
                Token::Keyword => style(0x0033B3),
                Token::String => style(0x067D17),
                Token::Escape => style(0x0037A6),
                Token::Number => style(0x1750EB),
                Token::Comment | Token::DocComment => italic(0x8C8C8C),
                Token::Function => style(0x00627A),
                Token::FunctionCall | Token::Type => style(fg),
                Token::Constant => italic(0x871094),
                Token::Property => style(0x871094),
                Token::Metadata => style(0x9E880D),
                Token::Tag => style(0x0033B3),
                Token::Attribute => style(0x174AD4),
                Token::Heading => bold(0x0033B3),
                Token::Link => style(0x006DCC),
                Token::Strong => bold(fg),
                Token::Emphasis => italic(fg),
                Token::Punctuation | Token::Operator => style(fg),
                // `RustDefault.xml` in the Rust plugin
                Token::Struct => style(0xA04908),
                Token::Enum => italic(0xA04908),
                Token::EnumVariant => italic(0x0E7FC5),
                Token::Crate => style(0x835DDA),
                Token::Macro => style(0xC27203),
                Token::SelfValue => style(0x1C5DFE),
                Token::QOperator => bold(0x1C5DFE),
            }),
        }
    }

    /// Catppuccin Mocha (no italics) from `catppuccin/jetbrains` (`editor.tera`,
    /// `ui.theme.tera`). Mixed colors are computed in advance, like
    /// `color | mix(color=base, amount=a)`.
    pub fn mocha() -> Self {
        const BASE: u32 = 0x1E1E2E;
        const SURFACE0: u32 = 0x313244;
        const SURFACE2: u32 = 0x585B70;
        const OVERLAY0: u32 = 0x6C7086;
        const OVERLAY2: u32 = 0x9399B2;
        const TEXT: u32 = 0xCDD6F4;
        const LAVENDER: u32 = 0xB4BEFE;
        const BLUE: u32 = 0x89B4FA;
        const SKY: u32 = 0x89DCEB;
        const TEAL: u32 = 0x94E2D5;
        const GREEN: u32 = 0xA6E3A1;
        const YELLOW: u32 = 0xF9E2AF;
        const PEACH: u32 = 0xFAB387;
        const RED: u32 = 0xF38BA8;
        const MAUVE: u32 = 0xCBA6F7;
        const PINK: u32 = 0xF5C2E7;
        let base = rgb(BASE);
        Self {
            dark: true,
            editor_bg: base,
            editor_fg: rgb(TEXT),
            line_number: rgb(OVERLAY0),
            line_number_caret_row: rgb(LAVENDER),
            // Like Islands Dark
            row_highlight: [0xFF, 0xFF, 0xFF, 0x12],
            // Yellow with 0.5
            gutter_highlight: rgb(0x8C806E),
            indent_guide: rgb(SURFACE0),
            // Inherited from Darcula
            separator_wave: rgb(0x555555),
            // Like Islands Dark
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            tree_selection: rgb(0x353748),
            // Between base and selected, since surface0 is almost the same as selected
            tree_selection_inactive: rgb(0x2A2B3C),
            commit_graph: rgb(TEAL),
            message_bg: rgb(GREEN),
            warning_bg: rgb(YELLOW),
            message_fg: base,
            file_added: rgb(GREEN),
            file_deleted: rgb(SURFACE2),
            file_modified: rgb(BLUE),
            // Green, red and blue with 0.25 for the background and 0.2 for the stripe
            inserted: DiffColors::new(rgb(0x404F4B), rgb(0x394545), base),
            deleted: DiffColors::new(rgb(0x53394C), rgb(0x493446), base),
            modified: DiffColors::new(rgb(0x394461), rgb(0x333C57), base),
            syntax: Token::ALL.map(|t| match t {
                Token::Keyword => style(MAUVE),
                Token::String => style(GREEN),
                Token::Escape => style(PINK),
                Token::Number => style(PEACH),
                Token::Comment | Token::DocComment => style(OVERLAY2),
                Token::Function | Token::FunctionCall => style(BLUE),
                Token::Type => style(YELLOW),
                Token::Constant => style(PEACH),
                Token::Property => style(TEXT),
                Token::Metadata => style(YELLOW),
                Token::Tag => style(TEAL),
                Token::Attribute => style(YELLOW),
                Token::Heading => bold(RED),
                Token::Link => style(LAVENDER),
                Token::Strong => bold(RED),
                Token::Emphasis => style(RED),
                Token::Punctuation => style(OVERLAY2),
                Token::Operator | Token::QOperator => style(SKY),
                // The scheme has no Rust keys, so as before
                Token::Struct | Token::Enum | Token::EnumVariant => style(YELLOW),
                Token::Crate => style(TEXT),
                Token::Macro => style(BLUE),
                Token::SelfValue => style(MAUVE),
            }),
        }
    }

    pub fn diff(&self, diff_type: DiffType) -> &DiffColors {
        match diff_type {
            DiffType::Inserted => &self.inserted,
            DiffType::Deleted => &self.deleted,
            DiffType::Modified => &self.modified,
        }
    }

    pub fn syntax(&self, token: Token) -> SyntaxStyle {
        self.syntax[token as usize]
    }
}

/// `ColorUtil.mix(background, editorBg, 0.6)`. None of the schemes set FOREGROUND on
/// the diff keys, so the ignored color is always derived.
fn ignored_color(background: Rgba, editor_bg: Rgba) -> Rgba {
    mix(background, editor_bg, 0.6)
}

/// `MixedColorProducer`: `c0 + (int) Math.round(ratio * (c1 - c0))` per channel.
fn mix(c0: Rgba, c1: Rgba, ratio: f64) -> Rgba {
    let mut out = [0; 4];
    for i in 0..4 {
        let (a, b) = (c0[i] as f64, c1[i] as f64);
        // Java `Math.round` is floor(x + 0.5), also for negative numbers
        out[i] = (a + (ratio * (b - a) + 0.5).floor()) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_in_dark() {
        let t = Theme::dark();
        assert_eq!(t.inserted.ignored, rgb(0x1F2B26));
        assert_eq!(t.deleted.ignored, rgb(0x2C2D2E));
        assert_eq!(t.modified.ignored, rgb(0x25323E));
    }

    #[test]
    fn ignored_in_light() {
        let t = Theme::light();
        assert_eq!(t.inserted.ignored, rgb(0xE5F5E5));
        assert_eq!(t.deleted.ignored, rgb(0xEFEFEF));
        assert_eq!(t.modified.ignored, rgb(0xE7EFFA));
    }

    #[test]
    fn mix_rounds_like_java() {
        // 0.6 * -5 = -3.0, 0.6 * -1 = -0.6 gives -1, 0.5 * -1 = -0.5 gives 0 in Java
        assert_eq!(mix([10, 0, 0, 255], [5, 0, 0, 255], 0.6)[0], 7);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.6)[0], 0);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.5)[0], 1);
    }
}
