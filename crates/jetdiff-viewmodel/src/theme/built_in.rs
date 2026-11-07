//! The built-in theme. The diff is Catppuccin Mocha (no italics) from `catppuccin/jetbrains`
//! (`editor.tera`, `ui.theme.tera`), with mixed colors computed in advance, like
//! `color | mix(color=base, amount=a)`. The palette is from the walkthrough's design.

use jetdiff_syntax::Token;

use super::syntax_style::{bold, style};
use super::{DiffColors, Palette, Pill, Theme, rgb};

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

impl Theme {
    pub fn built_in() -> Self {
        let base = rgb(BASE);
        Self {
            editor_bg: base,
            editor_fg: rgb(TEXT),
            line_number: rgb(OVERLAY0),
            cursor_row_line_number: rgb(LAVENDER),
            // Like IntelliJ's Islands Dark
            cursor_row_background: [0xFF, 0xFF, 0xFF, 0x12],
            // Yellow with 0.5
            cursor_row_gutter: rgb(0x8C806E),
            indent_guide: rgb(SURFACE0),
            // Inherited from Darcula
            fold_wave: rgb(0x555555),
            // Like IntelliJ's Islands Dark
            scrollbar_thumb: [0x80, 0x80, 0x80, 0x59],
            tree_selection: rgb(0x353748),
            // Between base and selected, since surface0 is almost the same as selected
            tree_selection_inactive: rgb(0x2A2B3C),
            commit_graph: rgb(TEAL),
            file_added: rgb(GREEN),
            file_deleted: rgb(SURFACE2),
            file_modified: rgb(BLUE),
            // Green, red and blue with 0.25 for the background and 0.2 for the hunk minimap
            added: DiffColors::new(rgb(0x404F4B), rgb(0x394545), base),
            deleted: DiffColors::new(rgb(0x53394C), rgb(0x493446), base),
            modified: DiffColors::new(rgb(0x394461), rgb(0x333C57), base),
            palette: palette(),
            syntax: Token::ALL.map(|token| match token {
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
                // The scheme has no Rust keys
                Token::Struct | Token::Enum | Token::EnumVariant => style(YELLOW),
                Token::Crate => style(TEXT),
                Token::Macro => style(BLUE),
                Token::SelfValue => style(MAUVE),
            }),
        }
    }
}

fn palette() -> Palette {
    let pill = |background, text| Pill {
        background: rgb(background),
        text: rgb(text),
    };
    Palette {
        page: rgb(0x16181f),
        surface: rgb(0x1c1f28),
        border: rgb(0x2a2e3b),
        selected: rgb(0x2b3042),
        code_background: rgb(0x262a36),
        file_header: rgb(0x262a36),
        strong: rgb(0xffffff),
        primary: rgb(0xe6e8ef),
        body: rgb(0xd4d7e1),
        secondary: rgb(0xc9cdd9),
        muted: rgb(0x9aa0b4),
        step_number: rgb(0x7d849a),
        accent: rgb(0x9cc4ff),
        button: rgb(0x6fa8ff),
        button_text: rgb(0x0f1420),
        read: rgb(0x7fd4a0),
        key_dot: rgb(0x6fa8ff),
        added_pill: pill(0x1f3a2c, 0x7fd4a0),
        deleted_pill: pill(0x3e1f27, 0xf5a3b6),
        decision_tag: pill(0x3a3016, 0xf3cf75),
        divergence_tag: pill(0x1d3050, 0x9cc4ff),
        limitation_tag: pill(0x3f2818, 0xf5ad80),
        untested_tag: pill(0x3e1f2a, 0xf5a3b6),
        // Upkeep has no color of its own in the design, so it gets the quiet one
        upkeep_tag: pill(0x262a36, 0xd4d7e1),
        // Starts from the limitation tag
        warning: pill(0x3f2818, 0xf5ad80),
        key_label_background: rgb(0x262a36),
        key_label_border: rgb(0x30354a),
        tree_guide_line: rgb(0x2f3445),
    }
}
