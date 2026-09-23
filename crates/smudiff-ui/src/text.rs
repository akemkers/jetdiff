//! Fontvalg og tekstmål. GPUI faller tilbake til Helvetica for ukjente familier
//! (`TextSystem::resolve_font`), som ikke er monospace. Derfor velges familien her.
//! Manglende glyfer (CJK, emoji) håndteres av CoreText sin font-kaskade ved shaping.

use std::borrow::Cow;
use std::path::PathBuf;

use gpui::{App, Font, FontFeatures, Pixels, SharedString, Window, font, px};

use crate::metrics::{FONT_SIZE, LINE_NUMBER_FONT_SIZE, LINE_SPACING, LineMetrics, line_metrics};

pub const PREFERRED_FAMILY: &str = "JetBrains Mono";
pub const FALLBACK_FAMILY: &str = "Menlo";

pub fn pick_family(installed: &[String]) -> &'static str {
    if installed.iter().any(|n| n == PREFERRED_FAMILY) {
        PREFERRED_FAMILY
    } else {
        FALLBACK_FAMILY
    }
}

/// JetBrains Mono er sjelden installert i systemet, men ligger i JBR-en til alle JetBrains-IDE-er.
/// Lastes derfra hvis den finnes, så tekstmålene blir de samme som i IDE-en.
pub fn load_font(cx: &App) -> &'static str {
    let ts = cx.text_system();
    if pick_family(&ts.all_font_names()) == PREFERRED_FAMILY {
        return PREFERRED_FAMILY;
    }
    for path in bundled_jetbrains_mono() {
        if let Ok(bytes) = std::fs::read(&path)
            && ts.add_fonts(vec![Cow::Owned(bytes)]).is_ok()
            && pick_family(&ts.all_font_names()) == PREFERRED_FAMILY
        {
            return PREFERRED_FAMILY;
        }
    }
    FALLBACK_FAMILY
}

fn bundled_jetbrains_mono() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    let mut found = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry
                .path()
                .join("Contents/jbr/Contents/Home/lib/fonts/JetBrainsMono-Regular.ttf");
            if path.is_file() {
                found.push(path);
            }
        }
    }
    found.sort();
    found
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
    pub fn new(family: &'static str, window: &Window) -> Self {
        let font = Font {
            // Ingen ligaturer i diff, som IntelliJ sin default
            features: FontFeatures::disable_ligatures(),
            ..font(family)
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

        let font_size = px(FONT_SIZE);
        let number_font_size = px(LINE_NUMBER_FONT_SIZE);
        let metrics = line_metrics(ascent(font_size), descent(font_size), 0., LINE_SPACING);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velger_jetbrains_mono_bare_hvis_installert() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_family(&names(&["Menlo", "Monaco"])), FALLBACK_FAMILY);
        assert_eq!(
            pick_family(&names(&["Menlo", "JetBrains Mono"])),
            PREFERRED_FAMILY
        );
        assert_eq!(pick_family(&[]), FALLBACK_FAMILY);
    }
}
