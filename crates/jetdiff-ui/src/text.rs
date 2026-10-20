//! Innebygd JetBrains Mono og tekstmål. Manglende glyfer (CJK, emoji) håndteres av CoreText sin font-kaskade ved shaping.

use std::borrow::Cow;

use gpui::{App, Font, FontFeatures, Pixels, SharedString, Window, font, px};

use jetdiff_viewmodel::metrics::{LINE_NUMBER_FONT_DELTA, LineMetrics, line_metrics};

pub(crate) const FAMILY: &str = "JetBrains Mono";

/// Samme versjon som i JBR-en til IntelliJ, så tekstmålene blir de samme som i IDE-en.
const FONTS: [&[u8]; 4] = [
    include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Italic.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../fonts/JetBrainsMono-BoldItalic.ttf"),
];

pub fn load_font(cx: &App) {
    cx.text_system()
        .add_fonts(FONTS.map(Cow::Borrowed).to_vec())
        .expect("innebygd font skal kunne lastes");
}

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font: Font,
    pub font_size: Pixels,
    pub metrics: LineMetrics,
    /// GPUI sentrerer glyfene i linjehøyden. Forskyvningen legges til linjetoppen så
    /// baseline havner der IntelliJ har den.
    pub text_dy: f32,
    pub char_width: f32,
    pub number_font_size: Pixels,
    /// Linjenumrene har samme baseline som teksten (`getLineNumberAscent` = editorens ascent).
    pub number_dy: f32,
    pub number_char_width: f32,
}

impl TextStyle {
    pub fn new(size: f32, window: &Window) -> Self {
        let font = Font {
            // Ingen ligaturer i diff, som IntelliJ sin default
            features: FontFeatures::disable_ligatures(),
            ..font(FAMILY)
        };
        let ts = window.text_system();
        let id = ts.resolve_font(&font);
        let ascent = |size| f32::from(ts.ascent(id, size));
        let descent = |size| f32::from(ts.descent(id, size)).abs();
        let digit = |size| {
            ts.advance(id, size, '0')
                .map(|s| f32::from(s.width))
                .unwrap_or(f32::from(size) * 0.6)
        };
        // Der GPUI legger baseline når en linje males med en gitt linjehøyde
        let gpui_baseline = |size, lh: f32| (lh - ascent(size) - descent(size)) / 2. + ascent(size);

        let font_size = px(size);
        let number_font_size = px(size - LINE_NUMBER_FONT_DELTA);
        let metrics = line_metrics(ascent(font_size), descent(font_size));
        let lh = metrics.line_height;
        Self {
            text_dy: metrics.baseline - gpui_baseline(font_size, lh),
            number_dy: metrics.baseline - gpui_baseline(number_font_size, lh),
            char_width: digit(font_size),
            number_char_width: digit(number_font_size),
            font,
            font_size,
            number_font_size,
            metrics,
        }
    }

    pub fn line_height(&self) -> f32 {
        self.metrics.line_height
    }

    /// Nøkkel for cachen: alt en shapet linje avhenger av utenom teksten.
    pub fn cache_key(&self) -> (SharedString, u32) {
        (
            self.font.family.clone(),
            f32::from(self.font_size).to_bits(),
        )
    }
}
