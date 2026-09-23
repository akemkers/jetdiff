//! Linjehøyde og baseline regnet ut som IntelliJ. `EditorView.createNewMetrics` tar
//! `FontMetrics.getHeight()` fra Java og ganger med linjeavstanden
//! (`FontPreferences.DEFAULT_LINE_SPACING = 1.2`), rundet opp.

/// `FontPreferences.DEFAULT_FONT_SIZE` med JetBrains Mono.
pub const FONT_SIZE: f32 = 13.;

/// `editor.gutter.linenumber.font.size.delta`: linjenumrene er 1pt mindre enn teksten.
pub const LINE_NUMBER_FONT_SIZE: f32 = FONT_SIZE - 1.;

/// `FontPreferences.DEFAULT_LINE_SPACING`.
pub const LINE_SPACING: f32 = 1.2;

/// `FontDesignMetrics.roundingUpValue`: Java runder ascent og descent med `(int)(0.95 + x)`.
const ROUNDING_UP: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMetrics {
    pub line_height: f32,
    /// Fra toppen av linja.
    pub baseline: f32,
}

/// Ascent og descent i piksler, begge positive. `leading` er linjeavstand fra fonten, 0 for
/// både JetBrains Mono og Menlo.
pub fn line_metrics(ascent: f32, descent: f32, leading: f32, line_spacing: f32) -> LineMetrics {
    let java_ascent = (ROUNDING_UP + ascent) as i32;
    let java_descent = (ROUNDING_UP + descent) as i32;
    let height = java_ascent + (ROUNDING_UP + descent + leading) as i32;
    // Java regner i float, så 25 * 1.2f blir 30.000002 og rundes opp til 31. f32 gir samme svar.
    let line_height = ((height as f32 * line_spacing).ceil() as i32).max(1);
    // `newDescent = descent + (lineHeight - fontMetricsHeight) / 2`, heltallsdivisjon
    let new_descent = java_descent + (line_height - height) / 2;
    LineMetrics {
        line_height: line_height as f32,
        baseline: (line_height - new_descent) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jetbrains_mono_13() {
        // hhea: ascent 1020, descent 300 av 1000
        let m = line_metrics(13.26, 3.9, 0., LINE_SPACING);
        assert_eq!(m.line_height, 22.);
        assert_eq!(m.baseline, 16.);
    }

    #[test]
    fn menlo_13() {
        // hhea: ascent 1901, descent 483 av 2048
        let m = line_metrics(1901. / 2048. * 13., 483. / 2048. * 13., 0., LINE_SPACING);
        assert_eq!(m.line_height, 21.);
        assert_eq!(m.baseline, 15.);
    }

    #[test]
    fn float_avrunding_som_java() {
        // 25 * 1.2f = 30.000002, mens 20 * 1.2f blir nøyaktig 24
        assert_eq!(line_metrics(20., 5., 0., 1.2).line_height, 31.);
        assert_eq!(line_metrics(15., 4.1, 0., 1.2).line_height, 24.);
        assert_eq!(line_metrics(10., 3., 0., 1.).line_height, 13.);
    }
}
