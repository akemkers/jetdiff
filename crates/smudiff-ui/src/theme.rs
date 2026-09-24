//! `smudiff_view::Theme` oversatt til GPUI-farger én gang, så paint slipper konvertering.

use gpui::Hsla;
use smudiff_view::theme::DiffColors;
use smudiff_view::{DiffType, Rgba, Theme};

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
pub struct UiTheme {
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
    inserted: UiDiffColors,
    deleted: UiDiffColors,
    modified: UiDiffColors,
}

impl UiTheme {
    pub fn new(theme: &Theme, dark: bool) -> Self {
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
            // Grå tommel med alfa, som macOS-scrollbaren i IntelliJ
            scrollbar_thumb: if dark {
                hsla([0x80, 0x80, 0x80, 0x59])
            } else {
                hsla([0x00, 0x00, 0x00, 0x33])
            },
            inserted: (&theme.inserted).into(),
            deleted: (&theme.deleted).into(),
            modified: (&theme.modified).into(),
        }
    }

    pub fn dark() -> Self {
        Self::new(&Theme::dark(), true)
    }

    pub fn light() -> Self {
        Self::new(&Theme::light(), false)
    }

    pub fn diff(&self, diff_type: DiffType) -> &UiDiffColors {
        match diff_type {
            DiffType::Inserted => &self.inserted,
            DiffType::Deleted => &self.deleted,
            DiffType::Modified => &self.modified,
        }
    }
}
