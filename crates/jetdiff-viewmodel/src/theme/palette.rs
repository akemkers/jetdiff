//! The colors around the diff: the page, surfaces, text, pills and key labels. Dark, and every text
//! color at least 4.5:1 against the background it is on.

use super::Rgba;

/// Generic over the color, so a view can convert the palette once into its own colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette<Color = Rgba> {
    pub page: Color,
    /// The contents and the cards.
    pub surface: Color,
    pub border: Color,
    pub selected: Color,
    /// Inline code in prose.
    pub code_background: Color,
    /// Lighter than the code, so each file starts with a bar of its own.
    pub file_header: Color,
    /// Headings, bold text and the selection.
    pub strong: Color,
    pub primary: Color,
    pub body: Color,
    pub secondary: Color,
    pub muted: Color,
    pub step_number: Color,
    pub accent: Color,
    pub button: Color,
    pub button_text: Color,
    pub read: Color,
    pub key_dot: Color,
    pub added_pill: Pill<Color>,
    pub deleted_pill: Pill<Color>,
    /// The note tags differ in lightness, not only in hue.
    pub decision_tag: Pill<Color>,
    pub divergence_tag: Pill<Color>,
    pub limitation_tag: Pill<Color>,
    pub untested_tag: Pill<Color>,
    pub upkeep_tag: Pill<Color>,
    pub key_label_background: Color,
    pub key_label_border: Color,
    pub tree_guide_line: Color,
}

/// A rounded label: text on a background of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pill<Color = Rgba> {
    pub background: Color,
    pub text: Color,
}

impl<Color: Copy> Palette<Color> {
    pub fn map<Other>(&self, convert: impl Fn(Color) -> Other) -> Palette<Other> {
        let convert = &convert;
        Palette {
            page: convert(self.page),
            surface: convert(self.surface),
            border: convert(self.border),
            selected: convert(self.selected),
            code_background: convert(self.code_background),
            file_header: convert(self.file_header),
            strong: convert(self.strong),
            primary: convert(self.primary),
            body: convert(self.body),
            secondary: convert(self.secondary),
            muted: convert(self.muted),
            step_number: convert(self.step_number),
            accent: convert(self.accent),
            button: convert(self.button),
            button_text: convert(self.button_text),
            read: convert(self.read),
            key_dot: convert(self.key_dot),
            added_pill: self.added_pill.map(convert),
            deleted_pill: self.deleted_pill.map(convert),
            decision_tag: self.decision_tag.map(convert),
            divergence_tag: self.divergence_tag.map(convert),
            limitation_tag: self.limitation_tag.map(convert),
            untested_tag: self.untested_tag.map(convert),
            upkeep_tag: self.upkeep_tag.map(convert),
            key_label_background: convert(self.key_label_background),
            key_label_border: convert(self.key_label_border),
            tree_guide_line: convert(self.tree_guide_line),
        }
    }
}

impl<Color: Copy> Pill<Color> {
    fn map<Other>(&self, convert: &impl Fn(Color) -> Other) -> Pill<Other> {
        Pill {
            background: convert(self.background),
            text: convert(self.text),
        }
    }
}
