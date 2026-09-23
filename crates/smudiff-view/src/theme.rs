//! Farger fra IntelliJ sine default-temaer i New UI: "Islands Dark" og "Islands Light"
//! (`LafManagerImpl.DefaultNewUiThemeStrategy`). Dark bruker editor-skjemaet
//! `IslandSchemeDark.xml` med Darcula som forelder. Light bruker `expUI_lightScheme.xml` med
//! Default som forelder.

use crate::model::DiffType;

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
pub struct Theme {
    pub editor_bg: Rgba,
    pub editor_fg: Rgba,
    /// `EDITOR_GUTTER_BACKGROUND` er tom i begge temaene, så gutteren arver editor-bakgrunnen.
    pub gutter_bg: Rgba,
    /// `DiffDrawUtil.getDividerColor`: divideren males med gutter-bakgrunnen.
    pub divider_bg: Rgba,
    pub line_number: Rgba,
    pub line_number_caret_row: Rgba,
    pub indent_guide: Rgba,
    /// `DIFF_SEPARATOR_WAVE`, streken over foldede områder.
    pub separator_wave: Rgba,
    pub inserted: DiffColors,
    pub deleted: DiffColors,
    pub modified: DiffColors,
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
            indent_guide: rgb(0x323438),
            separator_wave: rgb(0x555555),
            inserted: DiffColors::new(rgb(0x294436), rgb(0x447152), bg),
            deleted: DiffColors::new(rgb(0x484A4A), rgb(0x656E76), bg),
            modified: DiffColors::new(rgb(0x385570), rgb(0x43698D), bg),
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
            indent_guide: rgb(0xEBECF0),
            separator_wave: rgb(0xD0D0D0),
            inserted: DiffColors::new(rgb(0xBEE6BE), rgb(0xAADEAA), bg),
            deleted: DiffColors::new(rgb(0xD6D6D6), rgb(0xC8C8C8), bg),
            modified: DiffColors::new(rgb(0xC2D8F2), rgb(0xB6D2F2), bg),
        }
    }

    pub fn diff(&self, diff_type: DiffType) -> &DiffColors {
        match diff_type {
            DiffType::Inserted => &self.inserted,
            DiffType::Deleted => &self.deleted,
            DiffType::Modified => &self.modified,
        }
    }
}

/// `ColorUtil.mix(background, editorBg, 0.6)`. Ingen av skjemaene setter FOREGROUND på
/// diff-nøklene, så ignored-fargen er alltid avledet.
fn ignored_color(background: Rgba, editor_bg: Rgba) -> Rgba {
    mix(background, editor_bg, 0.6)
}

/// `MixedColorProducer`: `c0 + (int) Math.round(ratio * (c1 - c0))` per kanal.
pub fn mix(c0: Rgba, c1: Rgba, ratio: f64) -> Rgba {
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
