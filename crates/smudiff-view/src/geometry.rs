//! Horisontal layout for én side. Venstre side er speilet om divideren, som i IntelliJ:
//! `[tekst | gutter] [divider] [gutter | tekst]`, med scrollbaren over tekstens ytterkant.
//! Gutter-koordinater er lokale for høyre (uspeilede) editor: 0 er kanten mot divideren,
//! `width` er kanten mot teksten.

use std::ops::Range;

use smudiff_core::side::Side;

use crate::divider::DIVIDER_WIDTH;
use crate::model::FileDiff;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GutterLayout {
    pub width: f32,
    /// Linjenummer-området, lokalt.
    pub numbers: (f32, f32),
    /// `getWhitespaceSeparatorOffset`: herfra og ut mot teksten får ignored-farge.
    pub separator: f32,
}

/// Minste bredde på linjenummer-området: breakpoint-ikonet i New UI er 14px
/// (`getInitialLineNumberWidth`).
pub const MIN_NUMBERS_WIDTH: f32 = 14.;

/// `EditorGutterLayout.createNewUILayout` for en read-only diff-editor, fra divideren og ut:
/// tomt annotasjonsområde 4, gap 4, linjenumre, gap 4, ingen ikoner, folding 9 + 2,
/// ekstra free painters 8 og gap 1. `numbers_width` er `stringWidth` av største linjenummer.
pub fn gutter_layout(numbers_width: f32) -> GutterLayout {
    let numbers = numbers_width.round().max(MIN_NUMBERS_WIDTH);
    let start = 4. + 4.;
    let width = start + numbers + 4. + 11. + 8. + 1.;
    GutterLayout {
        width,
        numbers: (start, start + numbers),
        // `getWhitespaceSeparatorOffset` i New UI
        separator: width - 3.,
    }
}

/// Scrollbaren i `EditorMarkupModelImpl`: 14 + tynt gap 2 + `minMarkHeight` 2. Den er
/// gjennomsiktig og ligger over teksten (`editor.transparent.scrollbar`).
pub const SCROLLBAR_THICKNESS: f32 = 18.;
/// Brede markører starter ved `minMarkHeight + 2` og er 14px.
const MARKER_X: (f32, f32) = (4., 18.);
/// Mac-tommelen er maks 10px, sentrert i de 14px: `2 + minMark + 14/2 - w/2`.
const THUMB_X: (f32, f32) = (6., 16.);

#[derive(Debug, Clone, PartialEq)]
pub struct SideGeometry {
    pub side: Side,
    /// Venstre side i et par. En side alene er aldri speilet.
    pub mirrored: bool,
    /// Hele tekstområdet. Scrollbaren ligger over den ytterste delen.
    pub text: Range<f32>,
    /// Der teksten starter. Venstre side har scrollbaren på innsiden, så teksten flyttes forbi den.
    pub text_x: f32,
    pub gutter: Range<f32>,
    pub scrollbar: Range<f32>,
    pub gutter_layout: GutterLayout,
}

impl SideGeometry {
    /// `x` er venstre kant av siden og `width` hele bredden den får.
    pub fn new(
        side: Side,
        mirrored: bool,
        x: f32,
        width: f32,
        gutter_layout: GutterLayout,
    ) -> Self {
        let end = x + width;
        let gw = gutter_layout.width.min(width);
        let (text, gutter) = if mirrored {
            (x..end - gw, end - gw..end)
        } else {
            (x + gw..end, x..x + gw)
        };
        let sw = SCROLLBAR_THICKNESS.min(text.end - text.start);
        let scrollbar = if mirrored {
            text.start..text.start + sw
        } else {
            text.end - sw..text.end
        };
        let text_x = if mirrored {
            (text.start + sw).min(text.end)
        } else {
            text.start
        };
        Self {
            side,
            mirrored,
            text,
            text_x,
            gutter,
            scrollbar,
            gutter_layout,
        }
    }

    pub fn bounds(&self) -> Range<f32> {
        if self.mirrored {
            self.text.start..self.gutter.end
        } else {
            self.gutter.start..self.text.end
        }
    }

    /// Lokalt gutter-intervall til vindus-x. Speilet på venstre side.
    pub fn gutter_x(&self, local: Range<f32>) -> Range<f32> {
        if self.mirrored {
            self.gutter.end - local.end..self.gutter.end - local.start
        } else {
            self.gutter.start + local.start..self.gutter.start + local.end
        }
    }

    /// Lokalt scrollbar-intervall, 0 mot teksten, til vindus-x. Speilet på venstre side.
    fn scrollbar_x(&self, (start, end): (f32, f32)) -> Range<f32> {
        let clip = |r: Range<f32>| r.start.max(self.scrollbar.start)..r.end.min(self.scrollbar.end);
        if self.mirrored {
            let right = self.scrollbar.start + SCROLLBAR_THICKNESS;
            clip(right - end..right - start)
        } else {
            let left = self.scrollbar.end - SCROLLBAR_THICKNESS;
            clip(left + start..left + end)
        }
    }

    pub fn marker_x(&self) -> Range<f32> {
        self.scrollbar_x(MARKER_X)
    }

    pub fn thumb_x(&self) -> Range<f32> {
        self.scrollbar_x(THUMB_X)
    }

    /// X for linjenummeret. Høyre side høyrejusteres i området. Venstre er speilet to ganger
    /// i IntelliJ, så glyfene står normalt, men venstrejustert 1px før området
    /// (`textOffset = offset - areaWidth - 1`).
    pub fn number_x(&self, text_width: f32) -> f32 {
        let (start, end) = self.gutter_layout.numbers;
        if self.mirrored {
            self.gutter_x(start..end).start - 1.
        } else {
            self.gutter_x(start..end).end - text_width
        }
    }
}

/// Begge sidene for et vindu som starter på `x` og er `width` bredt. Med én side
/// (`FileDiff::single_side`) får den hele bredden og den andre null.
pub fn side_geometry(
    diff: &FileDiff,
    number_char_width: f32,
    x: f32,
    width: f32,
) -> [SideGeometry; 2] {
    let geom = |side: Side, mirrored, x, width| {
        // Siste linjenummer bestemmer bredden, som `getMaxLineNumberString`
        let digits = diff.line_count(side).to_string().len();
        let gutter = gutter_layout(digits as f32 * number_char_width);
        SideGeometry::new(side, mirrored, x, width, gutter)
    };
    if let Some(shown) = diff.single_side() {
        return [Side::Left, Side::Right].map(|side| {
            if side == shown {
                geom(side, false, x, width)
            } else {
                geom(side, false, x + width, 0.)
            }
        });
    }
    let side_width = ((width - DIVIDER_WIDTH) / 2.).floor().max(0.);
    [
        geom(Side::Left, true, x, side_width),
        geom(
            Side::Right,
            false,
            x + side_width + DIVIDER_WIDTH,
            width - side_width - DIVIDER_WIDTH,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_for_tre_sifre() {
        // "999" i JetBrains Mono 12 er 21.6px
        let g = gutter_layout(21.6);
        assert_eq!(g.numbers, (8., 30.));
        assert_eq!(g.width, 54.);
        assert_eq!(g.separator, 51.);
        // Én linje: breakpoint-plassen er minimum
        assert_eq!(gutter_layout(7.2).width, 46.);
    }

    const G: GutterLayout = GutterLayout {
        width: 40.,
        numbers: (10., 30.),
        separator: 37.,
    };

    #[test]
    fn venstre_er_speilet() {
        let g = SideGeometry::new(Side::Left, true, 0., 500., G);
        assert_eq!(g.text, 0. ..460.);
        assert_eq!(g.gutter, 460. ..500.);
        assert_eq!(g.bounds(), 0. ..500.);
        // Ytterste 3px mot teksten
        assert_eq!(g.gutter_x(G.separator..G.width), 460. ..463.);
        assert_eq!(g.number_x(99.), 469.);
        assert_eq!(g.scrollbar, 0. ..18.);
        assert_eq!(g.marker_x(), 0. ..14.);
        assert_eq!(g.thumb_x(), 2. ..12.);
    }

    #[test]
    fn hoyre_er_uspeilet() {
        let g = SideGeometry::new(Side::Right, false, 524., 500., G);
        assert_eq!(g.gutter, 524. ..564.);
        assert_eq!(g.text, 564. ..1024.);
        assert_eq!(g.gutter_x(G.separator..G.width), 561. ..564.);
        assert_eq!(g.number_x(16.), 538.);
        assert_eq!(g.scrollbar, 1006. ..1024.);
        assert_eq!(g.marker_x(), 1010. ..1024.);
        assert_eq!(g.thumb_x(), 1012. ..1022.);
    }

    #[test]
    fn ny_fil_far_hele_bredden() {
        use crate::ComparisonPolicy;
        let diff = FileDiff::new("", "a\n", ComparisonPolicy::Default).unwrap();
        let [left, right] = side_geometry(&diff, 7., 100., 800.);
        assert_eq!(right.bounds(), 100. ..900.);
        assert!(!right.mirrored);
        assert!(left.bounds().is_empty());
    }

    #[test]
    fn smalt_vindu_gir_ikke_negative_bredder() {
        let g = SideGeometry::new(Side::Right, false, 0., 30., G);
        assert_eq!(g.gutter, 0. ..30.);
        assert!(g.text.is_empty());
        assert!(g.scrollbar.is_empty());
        assert!(g.marker_x().is_empty());
    }
}
