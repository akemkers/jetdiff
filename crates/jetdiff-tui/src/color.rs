//! Temaets RGBA til terminalfarger. Terminalen har ikke alfa, så gjennomsiktige farger
//! blandes over bakgrunnen de ligger på.

use jetdiff_viewmodel::theme::DiffColors;
use jetdiff_viewmodel::{DiffType, Rgba, Theme};
use ratatui::style::Color;

pub fn color(c: Rgba) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

pub fn over(top: Rgba, bottom: Color) -> Color {
    let Color::Rgb(r, g, b) = bottom else {
        return bottom;
    };
    let a = top[3] as u16;
    let mix = |t: u8, b: u8| ((t as u16 * a + b as u16 * (255 - a)) / 255) as u8;
    Color::Rgb(mix(top[0], r), mix(top[1], g), mix(top[2], b))
}

pub fn faded(c: Rgba, opacity: f32) -> Rgba {
    [c[0], c[1], c[2], (c[3] as f32 * opacity) as u8]
}

pub fn diff_colors(theme: &Theme, diff_type: DiffType) -> &DiffColors {
    match diff_type {
        DiffType::Inserted => &theme.inserted,
        DiffType::Deleted => &theme.deleted,
        DiffType::Modified => &theme.modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blander_over_bakgrunnen() {
        let black = Color::Rgb(0, 0, 0);
        assert_eq!(over([255, 255, 255, 255], black), Color::Rgb(255, 255, 255));
        assert_eq!(over([255, 255, 255, 0], black), black);
        assert_eq!(over([255, 0, 0, 51], black), Color::Rgb(51, 0, 0));
    }
}
