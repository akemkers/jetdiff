//! `smudiff_view::Theme` oversatt til GPUI-farger én gang, så paint slipper konvertering.

use gpui::{Font, FontStyle, FontWeight, Hsla};
use smudiff_view::theme::DiffColors;
use smudiff_view::{DiffType, Rgba, SyntaxStyle, Theme, Token};

pub fn hsla(c: Rgba) -> Hsla {
    gpui::Rgba {
        r: c[0] as f32 / 255.,
        g: c[1] as f32 / 255.,
        b: c[2] as f32 / 255.,
        a: c[3] as f32 / 255.,
    }
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiDiffColors {
    pub background: Hsla,
    pub stripe: Hsla,
    pub ignored: Hsla,
}

impl From<&DiffColors> for UiDiffColors {
    fn from(c: &DiffColors) -> Self {
        Self {
            background: hsla(c.background),
            stripe: hsla(c.stripe),
            ignored: hsla(c.ignored),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiSyntax {
    pub color: Hsla,
    weight: FontWeight,
    style: FontStyle,
}

impl UiSyntax {
    fn new(s: SyntaxStyle) -> Self {
        Self {
            color: hsla(s.color),
            weight: if s.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            style: if s.italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
        }
    }

    pub fn font(&self, base: &Font) -> Font {
        Font {
            weight: self.weight,
            style: self.style,
            ..base.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTheme {
    /// Velger `_dark`-ikonene.
    pub dark: bool,
    pub editor_bg: Hsla,
    pub editor_fg: Hsla,
    pub gutter_bg: Hsla,
    pub divider_bg: Hsla,
    pub line_number: Hsla,
    pub line_number_caret_row: Hsla,
    pub caret: Hsla,
    pub caret_row: Hsla,
    pub indent_guide: Hsla,
    /// `DIFF_SEPARATOR_WAVE`.
    pub separator_wave: Hsla,
    pub scrollbar_thumb: Hsla,
    pub tree_selection: Hsla,
    pub file_added: Hsla,
    pub file_deleted: Hsla,
    pub file_modified: Hsla,
    inserted: UiDiffColors,
    deleted: UiDiffColors,
    modified: UiDiffColors,
    syntax: [UiSyntax; Token::ALL.len()],
}

impl UiTheme {
    fn new(theme: &Theme, dark: bool) -> Self {
        Self {
            dark,
            editor_bg: hsla(theme.editor_bg),
            editor_fg: hsla(theme.editor_fg),
            gutter_bg: hsla(theme.gutter_bg),
            divider_bg: hsla(theme.divider_bg),
            line_number: hsla(theme.line_number),
            line_number_caret_row: hsla(theme.line_number_caret_row),
            caret: hsla(theme.caret),
            caret_row: hsla(theme.caret_row),
            indent_guide: hsla(theme.indent_guide),
            separator_wave: hsla(theme.separator_wave),
            scrollbar_thumb: hsla(theme.scrollbar_thumb),
            tree_selection: hsla(theme.tree_selection),
            file_added: hsla(theme.file_added),
            file_deleted: hsla(theme.file_deleted),
            file_modified: hsla(theme.file_modified),
            inserted: (&theme.inserted).into(),
            deleted: (&theme.deleted).into(),
            modified: (&theme.modified).into(),
            syntax: Token::ALL.map(|t| UiSyntax::new(theme.syntax(t))),
        }
    }

    pub fn dark() -> Self {
        Self::new(&Theme::dark(), true)
    }

    pub fn light() -> Self {
        Self::new(&Theme::light(), false)
    }

    pub fn syntax(&self, token: Token) -> &UiSyntax {
        &self.syntax[token as usize]
    }

    pub fn diff(&self, diff_type: DiffType) -> &UiDiffColors {
        match diff_type {
            DiffType::Inserted => &self.inserted,
            DiffType::Deleted => &self.deleted,
            DiffType::Modified => &self.modified,
        }
    }
}
