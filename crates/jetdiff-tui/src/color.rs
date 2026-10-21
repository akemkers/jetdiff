//! Temaets RGBA til terminalfarger. Terminalen har ikke alfa, så gjennomsiktige farger
//! blandes over bakgrunnen de ligger på.

use jetdiff_viewmodel::Rgba;
use ratatui::style::Color;

pub fn color(rgba: Rgba) -> Color {
    Color::Rgb(rgba[0], rgba[1], rgba[2])
}

pub fn over(top: Rgba, bottom: Color) -> Color {
    let Color::Rgb(r, g, b) = bottom else {
        return bottom;
    };
    let alpha = top[3] as u16;
    let mix = |top_channel: u8, bottom_channel: u8| {
        ((top_channel as u16 * alpha + bottom_channel as u16 * (255 - alpha)) / 255) as u8
    };
    Color::Rgb(mix(top[0], r), mix(top[1], g), mix(top[2], b))
}

pub fn faded(rgba: Rgba, opacity: f32) -> Rgba {
    [rgba[0], rgba[1], rgba[2], (rgba[3] as f32 * opacity) as u8]
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
