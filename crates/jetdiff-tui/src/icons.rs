//! File type icons as Nerd Font characters, with the same selection as in the GPUI view. The colors are from
//! the SVGs in `jetdiff-ui/icons`, light and dark variant.

use jetdiff_viewmodel::Language;
use ratatui::style::Color;

pub struct Icon {
    pub glyph: char,
    pub color: Color,
}

impl Icon {
    fn new(glyph: char, light: u32, dark: u32, is_dark: bool) -> Self {
        Self {
            glyph,
            color: Color::from_u32(if is_dark { dark } else { light }),
        }
    }
}

pub fn folder_icon(dark: bool) -> Icon {
    Icon::new('\u{e5ff}', 0x6C707E, 0xCED0D6, dark)
}

/// `nf-dev-git_branch`, in the color of `branch.svg`.
pub fn branch_icon(dark: bool) -> Icon {
    Icon::new('\u{e725}', 0x6C707E, 0xCED0D6, dark)
}

pub fn file_icon(path: &str, dark: bool) -> Icon {
    use Language::*;
    let (glyph, light, dark_color) = match Language::from_path(path) {
        Some(Java) => ('\u{e738}', 0xE66D17, 0xC77D55),
        Some(JavaScript) => ('\u{e60c}', 0xFFAF0F, 0xF2C55C),
        Some(Json) => ('\u{e80b}', 0x834DF0, 0xB589EC),
        Some(Kotlin) => ('\u{e634}', 0x834DF0, 0xA571E6),
        Some(Markdown) => ('\u{e609}', 0x3574F0, 0x548AF7),
        Some(Toml) => ('\u{e6b2}', 0x3574F0, 0x548AF7),
        Some(Tsx | TypeScript) => ('\u{e628}', 0x3574F0, 0x548AF7),
        Some(Xml) => ('\u{e619}', 0xE66D17, 0xC77D55),
        Some(Yaml) => ('\u{e6a8}', 0xDB3B4B, 0xDB5C5C),
        Some(Rust) => ('\u{e7a8}', 0xE66D17, 0xC77D55),
        Some(Go) => ('\u{e627}', 0x007D9C, 0x00ADD8),
        None => ('\u{e64e}', 0x6C707E, 0xCED0D6),
    };
    Icon::new(glyph, light, dark_color, dark)
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthChar;

    use super::*;

    #[test]
    fn the_icons_are_one_column() {
        for path in ["a.rs", "a.kt", "a.ts", "a.txt", "a.go"] {
            assert_eq!(file_icon(path, true).glyph.width(), Some(1), "{path}");
        }
        assert_eq!(folder_icon(true).glyph.width(), Some(1));
        assert_eq!(branch_icon(true).glyph.width(), Some(1));
    }

    #[test]
    fn the_color_follows_the_theme() {
        assert_eq!(file_icon("a.rs", true).color, Color::Rgb(0xC7, 0x7D, 0x55));
        assert_eq!(file_icon("a.rs", false).color, Color::Rgb(0xE6, 0x6D, 0x17));
    }
}
