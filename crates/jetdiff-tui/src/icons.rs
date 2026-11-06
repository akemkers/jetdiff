//! File type icons as Nerd Font characters, with the same selection as in the GPUI view. The colors are from
//! the SVGs in `jetdiff-ui/icons`.

use jetdiff_viewmodel::Language;
use ratatui::style::Color;

pub struct Icon {
    pub glyph: char,
    pub color: Color,
}

impl Icon {
    fn new(glyph: char, color: u32) -> Self {
        Self {
            glyph,
            color: Color::from_u32(color),
        }
    }
}

pub fn folder_icon() -> Icon {
    Icon::new('\u{e5ff}', 0xCED0D6)
}

/// `nf-dev-git_branch`, in the color of `branch.svg`.
pub fn branch_icon() -> Icon {
    Icon::new('\u{e725}', 0xCED0D6)
}

pub fn file_icon(path: &str) -> Icon {
    use Language::*;
    let (glyph, color) = match Language::from_path(path) {
        Some(Java) => ('\u{e738}', 0xC77D55),
        Some(JavaScript) => ('\u{e60c}', 0xF2C55C),
        Some(Json) => ('\u{e80b}', 0xB589EC),
        Some(Kotlin) => ('\u{e634}', 0xA571E6),
        Some(Markdown) => ('\u{e609}', 0x548AF7),
        Some(Python) => ('\u{e606}', 0x548AF7),
        Some(Toml) => ('\u{e6b2}', 0x548AF7),
        Some(Tsx | TypeScript) => ('\u{e628}', 0x548AF7),
        Some(Xml) => ('\u{e619}', 0xC77D55),
        Some(Yaml) => ('\u{e6a8}', 0xDB5C5C),
        Some(Rust) => ('\u{e7a8}', 0xC77D55),
        Some(Go) => ('\u{e627}', 0x00ADD8),
        None => ('\u{e64e}', 0xCED0D6),
    };
    Icon::new(glyph, color)
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthChar;

    use super::*;

    #[test]
    fn the_icons_are_one_column() {
        for path in ["a.rs", "a.kt", "a.ts", "a.txt", "a.go", "a.py"] {
            assert_eq!(file_icon(path).glyph.width(), Some(1), "{path}");
        }
        assert_eq!(folder_icon().glyph.width(), Some(1));
        assert_eq!(branch_icon().glyph.width(), Some(1));
    }

    #[test]
    fn the_color_is_from_the_svg() {
        assert_eq!(file_icon("a.rs").color, Color::Rgb(0xC7, 0x7D, 0x55));
    }
}
