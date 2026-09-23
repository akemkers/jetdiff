//! Vertikal geometri for én side. Alt som går fra linje til y går hit, så folding kan
//! byttes inn her uten å røre scroll, divider og stripe. Uten folding er visuell linje
//! lik logisk linje.

use std::ops::Range;

/// `editor.virtual.lines`: diff-editorene kan scrolles fem linjer forbi slutten.
pub const ADDITIONAL_LINES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub height: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineLayout {
    line_count: usize,
    line_height: f32,
}

impl LineLayout {
    pub fn new(line_count: usize, line_height: f32) -> Self {
        assert!(line_height > 0.);
        Self {
            line_count: line_count.max(1),
            line_height,
        }
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    pub fn line_count(&self) -> usize {
        self.line_count
    }

    pub fn visual_line_count(&self) -> usize {
        self.line_count
    }

    /// Linjer forbi slutten mappes lineært, som `logicalToVisualPosition` i virtuelt rom.
    pub fn logical_to_visual(&self, line: usize) -> usize {
        line
    }

    pub fn visual_to_logical(&self, visual: usize) -> usize {
        visual
    }

    pub fn visual_line_to_y(&self, visual: usize) -> f32 {
        visual as f32 * self.line_height
    }

    /// `EditorCoordinateMapper.yToVisualLine`: alt over første linje er linje 0, og forbi
    /// slutten fortsetter den lineært.
    pub fn y_to_visual_line(&self, y: f32) -> usize {
        if y < self.line_height {
            return 0;
        }
        (y / self.line_height).floor() as usize
    }

    pub fn y_to_logical_line(&self, y: f32) -> usize {
        self.visual_to_logical(self.y_to_visual_line(y))
    }

    pub fn line_top(&self, line: usize) -> f32 {
        self.visual_line_to_y(self.logical_to_visual(line))
    }

    /// Eksklusiv bunn av linja.
    pub fn line_bottom(&self, line: usize) -> f32 {
        self.line_top(line) + self.line_height
    }

    /// `DiffDrawUtil.getGutterMarkerPaintRange`. Tom range gir `y1 == y2` på linjegrensen,
    /// og linje 0 flyttes 1px ned så 2px-båndet blir `[0, 2)`.
    pub fn marker_range(&self, lines: Range<usize>) -> (f32, f32) {
        if lines.is_empty() {
            let y = if lines.start == 0 {
                self.line_top(0) + 1.
            } else {
                self.line_bottom(lines.start - 1)
            };
            (y, y)
        } else {
            (self.line_top(lines.start), self.line_bottom(lines.end - 1))
        }
    }

    /// Y-intervallet en blokk dekker i editoren. Tom side blir 2px `[grense-1, grense+1)`,
    /// samme bånd som `DiffLineMarkerRenderer` og `DiffDefaultBorderRenderer` tegner.
    pub fn block_band(&self, lines: Range<usize>) -> (f32, f32) {
        let (y1, y2) = self.marker_range(lines);
        if y2 - y1 <= 2. {
            (y1 - 1., y1 + 1.)
        } else {
            (y1, y2)
        }
    }

    /// `EditorSizeManager.getPreferredHeight` med `additionalLinesCount`.
    pub fn content_height(&self) -> f32 {
        (self.visual_line_count() + ADDITIONAL_LINES) as f32 * self.line_height
    }

    pub fn max_scroll(&self, viewport_height: f32) -> f32 {
        (self.content_height() - viewport_height).max(0.)
    }

    pub fn clamp_scroll(&self, y: f32, viewport_height: f32) -> f32 {
        y.clamp(0., self.max_scroll(viewport_height))
    }

    /// Logiske linjer som er helt eller delvis synlige, lukket intervall som
    /// `DiffDividerDrawUtil.getPaintedInterval`.
    pub fn painted_lines(&self, scroll_y: f32, viewport_height: f32) -> (usize, usize) {
        (
            self.y_to_logical_line(scroll_y),
            self.y_to_logical_line(scroll_y + viewport_height),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_til_linje() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.y_to_visual_line(-5.), 0);
        assert_eq!(l.y_to_visual_line(19.9), 0);
        assert_eq!(l.y_to_visual_line(20.), 1);
        assert_eq!(l.y_to_visual_line(250.), 12);
    }

    #[test]
    fn marker_range_for_tom_og_ikke_tom() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.marker_range(0..0), (1., 1.));
        assert_eq!(l.marker_range(3..3), (60., 60.));
        assert_eq!(l.marker_range(2..4), (40., 80.));
        // Etter siste linje, f.eks. manglende '\n' på slutten
        assert_eq!(l.marker_range(10..10), (200., 200.));
    }

    #[test]
    fn block_band_gir_2px_for_tom_side() {
        let l = LineLayout::new(10, 20.);
        assert_eq!(l.block_band(0..0), (0., 2.));
        assert_eq!(l.block_band(3..3), (59., 61.));
        assert_eq!(l.block_band(3..5), (60., 100.));
    }

    #[test]
    fn scroll_klemmes_med_fem_ekstra_linjer() {
        let l = LineLayout::new(100, 20.);
        assert_eq!(l.content_height(), 2100.);
        assert_eq!(l.max_scroll(600.), 1500.);
        assert_eq!(l.clamp_scroll(-3., 600.), 0.);
        assert_eq!(l.clamp_scroll(9999., 600.), 1500.);
        assert_eq!(LineLayout::new(3, 20.).max_scroll(600.), 0.);
    }
}
