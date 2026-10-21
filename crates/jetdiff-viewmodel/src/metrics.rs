//! Line height and baseline computed like IntelliJ. `EditorView.createNewMetrics` takes
//! `FontMetrics.getHeight()` from Java and multiplies by the line spacing
//! (`FontPreferences.DEFAULT_LINE_SPACING = 1.2`), rounded up.

/// The size the side panel is drawn for. IntelliJ has 13 (`FontPreferences.DEFAULT_FONT_SIZE`).
pub const FONT_SIZE: f32 = 14.;

/// `editor.gutter.linenumber.font.size.delta`: the line numbers are 1pt smaller than the text.
pub const LINE_NUMBER_FONT_DELTA: f32 = 1.;

/// Zoom limits. IntelliJ's `EditorFontSizeAction` has 4..72, but under 8 is unreadable.
pub const MIN_FONT_SIZE: f32 = 8.;
pub const MAX_FONT_SIZE: f32 = 40.;

/// Cmd+Plus and Cmd+Minus go 1pt at a time, like `EditorIncreaseFontSize`.
pub fn zoomed(size: f32, steps: f32) -> f32 {
    (size + steps).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

/// `FontPreferences.DEFAULT_LINE_SPACING`.
const LINE_SPACING: f32 = 1.2;

/// `FontDesignMetrics.roundingUpValue`: Java rounds ascent and descent with `(int)(0.95 + x)`.
const ROUNDING_UP: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMetrics {
    pub line_height: f32,
    /// From the top of the line.
    pub baseline: f32,
}

/// Ascent and descent in pixels, both positive. The font's leading is 0 for both JetBrains Mono
/// and Menlo, so it is left out.
pub fn line_metrics(ascent: f32, descent: f32) -> LineMetrics {
    let java_ascent = (ROUNDING_UP + ascent) as i32;
    let java_descent = (ROUNDING_UP + descent) as i32;
    let height = java_ascent + java_descent;
    // Java computes in float, so 25 * 1.2f becomes 30.000002 and is rounded up to 31. f32 gives the same answer.
    let line_height = ((height as f32 * LINE_SPACING).ceil() as i32).max(1);
    // `newDescent = descent + (lineHeight - fontMetricsHeight) / 2`, integer division
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
    fn zoom_is_clamped() {
        assert_eq!(zoomed(FONT_SIZE, 1.), FONT_SIZE + 1.);
        assert_eq!(zoomed(FONT_SIZE, -1.), FONT_SIZE - 1.);
        assert_eq!(zoomed(MAX_FONT_SIZE, 1.), MAX_FONT_SIZE);
        assert_eq!(zoomed(MIN_FONT_SIZE, -1.), MIN_FONT_SIZE);
    }

    #[test]
    fn jetbrains_mono_13() {
        // hhea: ascent 1020, descent 300 of 1000
        let m = line_metrics(13.26, 3.9);
        assert_eq!(m.line_height, 22.);
        assert_eq!(m.baseline, 16.);
    }

    #[test]
    fn menlo_13() {
        // hhea: ascent 1901, descent 483 of 2048
        let m = line_metrics(1901. / 2048. * 13., 483. / 2048. * 13.);
        assert_eq!(m.line_height, 21.);
        assert_eq!(m.baseline, 15.);
    }

    #[test]
    fn float_rounding_like_java() {
        // 25 * 1.2f = 30.000002, while 20 * 1.2f becomes exactly 24
        assert_eq!(line_metrics(20., 5.).line_height, 31.);
        assert_eq!(line_metrics(15., 4.1).line_height, 24.);
    }
}
