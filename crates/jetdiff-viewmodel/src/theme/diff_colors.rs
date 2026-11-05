//! The colors of one hunk kind, with the ignored color derived as in IntelliJ.

use super::Rgba;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColors {
    /// Line background and inline highlight, and the fill in the divider.
    pub background: Rgba,
    /// `ERROR_STRIPE_COLOR` in the scrollbar.
    pub hunk_minimap: Rgba,
    /// Background for lines with inline highlight (`TextDiffTypeFactory.getIgnoredBackgroundColor`).
    pub ignored: Rgba,
}

impl DiffColors {
    pub(super) fn new(background: Rgba, hunk_minimap: Rgba, editor_bg: Rgba) -> Self {
        Self {
            background,
            hunk_minimap,
            ignored: ignored_color(background, editor_bg),
        }
    }
}

/// `ColorUtil.mix(background, editorBg, 0.6)`. The theme sets no FOREGROUND on the diff keys, so
/// the ignored color is always derived.
fn ignored_color(background: Rgba, editor_bg: Rgba) -> Rgba {
    mix(background, editor_bg, 0.6)
}

/// `MixedColorProducer`: `c0 + (int) Math.round(ratio * (c1 - c0))` per channel.
fn mix(c0: Rgba, c1: Rgba, ratio: f64) -> Rgba {
    let mut out = [0; 4];
    for i in 0..4 {
        let (a, b) = (c0[i] as f64, c1[i] as f64);
        // Java `Math.round` is floor(x + 0.5), also for negative numbers
        out[i] = (a + (ratio * (b - a) + 0.5).floor()) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_rounds_like_java() {
        // 0.6 * -5 = -3.0, 0.6 * -1 = -0.6 gives -1, 0.5 * -1 = -0.5 gives 0 in Java
        assert_eq!(mix([10, 0, 0, 255], [5, 0, 0, 255], 0.6)[0], 7);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.6)[0], 0);
        assert_eq!(mix([1, 0, 0, 255], [0, 0, 0, 255], 0.5)[0], 1);
    }
}
