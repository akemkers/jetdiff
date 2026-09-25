//! Trakten mellom sidene. Port av `DiffDividerDrawUtil` (`DividerPaintableHandlerImpl`,
//! `DividerPolygon.paint`) og `DiffDrawUtil.drawCurveTrapezium`. Koordinater er lokale for
//! divideren: x fra 0 til bredden, y fra toppen av editorene.

use crate::folding::{FoldId, Folding};
use crate::layout::{LineLayout, Viewport};
use crate::model::{DiffType, FileDiff};
use crate::separator::{PathSeg, connector};

/// `diff.divider.width` i registry.
pub const DIVIDER_WIDTH: f32 = 24.;

/// `DiffDrawUtil.CTRL_PROXIMITY_X`.
const CTRL_PROXIMITY_X: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cubic {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
    pub p3: Point,
}

impl Cubic {
    /// `DiffDrawUtil.makeCurve(forward = true)`: fra venstre y1 til høyre y2.
    pub fn new(width: f32, y1: f32, y2: f32) -> Self {
        let p = |x, y| Point { x, y };
        Self {
            p0: p(0., y1),
            p1: p(width * CTRL_PROXIMITY_X, y1),
            p2: p(width * (1. - CTRL_PROXIMITY_X), y2),
            p3: p(width, y2),
        }
    }

    pub fn reversed(&self) -> Self {
        Self {
            p0: self.p3,
            p1: self.p2,
            p2: self.p1,
            p3: self.p0,
        }
    }
}

/// Én blokk i divideren. Fyll: `top`, linje til `bottom.p3`, `bottom.reversed()`, lukk.
/// Tegn også `mid` med 1px strek i samme farge, så bratte bånd aldri blir tynnere enn 1px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DividerBand {
    pub block: usize,
    /// Full farge, også for ignored-blokker.
    pub diff_type: DiffType,
    pub top: Cubic,
    pub bottom: Cubic,
    pub mid: Cubic,
}

/// Blokker som er synlige i minst én editor. `scroll_y` er per side, `[venstre, høyre]`.
pub fn divider_bands(
    diff: &FileDiff,
    folding: &Folding,
    scroll_y: [f32; 2],
    viewport: Viewport,
    width: f32,
) -> Vec<DividerBand> {
    let layouts = folding.layouts(viewport.line_height);
    let painted = [
        layouts[0].painted_lines(scroll_y[0], viewport.height),
        layouts[1].painted_lines(scroll_y[1], viewport.height),
    ];

    let above =
        |b: &crate::model::Block| painted[0].0 > b.lines[0].end && painted[1].0 > b.lines[1].end;
    // `DividerPaintableHandlerImpl.process` hopper over blokker over viewporten. Endene
    // stiger på begge sider, så de kan hoppes over med binærsøk.
    let first = diff.blocks().partition_point(above);
    let mut bands = Vec::new();
    for (i, block) in diff.blocks().iter().enumerate().skip(first) {
        let [l, r] = &block.lines;
        if painted[0].1 < l.start && painted[1].1 < r.start {
            break;
        }
        let side = |s: usize| trapezium_side(&layouts[s], block.lines[s].clone(), scroll_y[s]);
        let (s1, e1) = side(0);
        let (s2, e2) = side(1);
        bands.push(DividerBand {
            block: i,
            diff_type: block.diff_type,
            top: Cubic::new(width, s1, s2),
            bottom: Cubic::new(width, e1 + 1., e2 + 1.),
            // Java: `(start + end) / 2` i int, og strek på heltalls-y dekker neste halve piksel
            mid: Cubic::new(width, mid_y(s1, e1), mid_y(s2, e2)),
        });
    }
    bands
}

/// Kobling mellom to kollapsede folds, strekes med `separator::stroke_width(hovered)`.
#[derive(Debug, Clone, PartialEq)]
pub struct DividerSeparator {
    pub id: FoldId,
    pub hovered: bool,
    pub path: Vec<PathSeg>,
}

/// `FoldingModelSupport.createVisibleSeparators`: koblinger for folds som er malt på minst
/// én side.
pub fn divider_separators(
    folding: &Folding,
    scroll_y: [f32; 2],
    viewport: Viewport,
    width: f32,
) -> Vec<DividerSeparator> {
    let lh = viewport.line_height;
    let layouts = folding.layouts(lh);
    let painted = [0, 1].map(|i| layouts[i].painted_lines(scroll_y[i], viewport.height));
    let mut result = Vec::new();
    for c in folding.connectors() {
        let [l1, l2] = c.lines;
        if painted[0].0 > l1 + 1 && painted[1].0 > l2 + 1 {
            continue;
        }
        if painted[0].1 < l1 && painted[1].1 < l2 {
            break;
        }
        let y = [0, 1].map(|i| layouts[i].line_top(c.lines[i]) - scroll_y[i]);
        result.push(DividerSeparator {
            id: c.id,
            hovered: c.hovered,
            path: connector(width, y[0], y[1], lh),
        });
    }
    result
}

/// `DividerPolygon.paint`: under 2px høyde blir `[start-1, start]`, ellers `[start, end-1]`.
/// Fyllet går til `end + 1`, så båndet dekker `[start, end)` eller 2px rundt grensen.
fn trapezium_side(layout: &LineLayout, lines: std::ops::Range<usize>, scroll_y: f32) -> (f32, f32) {
    let (y1, y2) = layout.marker_range(lines);
    let (y1, y2) = (y1 - scroll_y, y2 - scroll_y);
    if y2 - y1 < 2. {
        (y1 - 1., y1)
    } else {
        (y1, y2 - 1.)
    }
}

fn mid_y(start: f32, end: f32) -> f32 {
    ((start + end) / 2.).floor() + 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    const VP: Viewport = Viewport {
        height: 200.,
        line_height: 20.,
    };

    #[test]
    fn kurve_har_kontrollpunkter_pa_30_og_70_prosent() {
        let c = Cubic::new(24., 10., 50.);
        assert_eq!(c.p0, Point { x: 0., y: 10. });
        assert_eq!(
            c.p1,
            Point {
                x: 24. * 0.3,
                y: 10.
            }
        );
        assert_eq!(
            c.p2,
            Point {
                x: 24. * 0.7,
                y: 50.
            }
        );
        assert_eq!(c.p3, Point { x: 24., y: 50. });
        assert_eq!(c.reversed().p0, c.p3);
    }

    #[test]
    fn modified_blokk_med_kjente_tall() {
        // Venstre linje 2..4, høyre linje 2..5
        let d = diff("a\nb\nc\nd\ne\n", "a\nb\nx\ny\nz\ne\n");
        assert_eq!(d.blocks()[0].lines, [2..4, 2..5]);
        let bands = divider_bands(&d, &Folding::none(&d), [0., 10.], VP, 24.);
        assert_eq!(bands.len(), 1);
        let b = bands[0];
        assert_eq!(b.diff_type, DiffType::Modified);
        assert_eq!((b.top.p0.y, b.top.p3.y), (40., 30.));
        assert_eq!((b.bottom.p0.y, b.bottom.p3.y), (80., 90.));
        // (40 + 79) / 2 = 59, (30 + 89) / 2 = 59
        assert_eq!((b.mid.p0.y, b.mid.p3.y), (59.5, 59.5));
    }

    #[test]
    fn tom_side_blir_2px() {
        let d = diff("a\nb\n", "a\nx\ny\nb\n");
        assert_eq!(d.blocks()[0].lines, [1..1, 1..3]);
        let b = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
        assert_eq!(b.diff_type, DiffType::Inserted);
        // Grense på y=20: [19, 21)
        assert_eq!((b.top.p0.y, b.bottom.p0.y), (19., 21.));
        assert_eq!((b.top.p3.y, b.bottom.p3.y), (20., 60.));
        assert_eq!(b.mid.p0.y, 19.5);
    }

    #[test]
    fn tom_side_pa_forste_linje_flyttes_ned() {
        let d = diff("b\n", "x\nb\n");
        let b = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
        // `getGutterMarkerPaintRange` gir y=1 for linje 0, så båndet blir [0, 2)
        assert_eq!((b.top.p0.y, b.bottom.p0.y), (0., 2.));
    }

    #[test]
    fn blokker_utenfor_viewport_kuttes() {
        // Endringer på linje 5, 30 og 60
        let a = lines("a", 80);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [5, 30, 60] {
            b[i] = format!("x{i}");
        }
        let b = b.join("\n") + "\n";
        let d = diff(&a, &b);
        assert_eq!(d.blocks().len(), 3);
        // Viewport dekker linje 25..35
        let bands = divider_bands(&d, &Folding::none(&d), [500., 500.], VP, 24.);
        assert_eq!(bands.iter().map(|b| b.block).collect::<Vec<_>>(), vec![1]);
        // Synlig på bare én side holder
        let bands = divider_bands(&d, &Folding::none(&d), [0., 500.], VP, 24.);
        assert_eq!(
            bands.iter().map(|b| b.block).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn blokk_rett_under_kanten_tas_med() {
        // Lukket intervall: linja som starter på bunnkanten regnes som malt
        let d = diff(
            &lines("a", 20),
            &(lines("a", 10) + "x\n" + &lines("a", 20)[lines("a", 11).len()..]),
        );
        assert_eq!(d.blocks()[0].lines, [10..11, 10..11]);
        assert_eq!(
            divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.).len(),
            1
        );
        assert_eq!(
            divider_bands(&d, &Folding::none(&d), [1., 1.], VP, 24.).len(),
            1
        );
    }

    #[test]
    fn koblinger_for_synlige_folds() {
        use crate::folding::FoldSettings;
        // Endringer på linje 10, 40 og 90 av 100: folds 0..6, 15..36, 45..86 og 95..101
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        let f = Folding::new(&d, FoldSettings::default());
        let seps = divider_separators(&f, [0., 0.], VP, 24.);
        // Viewport på 10 visuelle linjer: fold 0 og 1 (visuell linje 0 og 10)
        assert_eq!(seps.len(), 2);
        assert_eq!(seps[0].path[0], PathSeg::Move(Point { x: -0.5, y: 10. }));
        assert_eq!(seps[1].path[0], PathSeg::Move(Point { x: -0.5, y: 210. }));
        // Delvis synlig fold er med, helt skjult er ikke
        let groups = |y: f32| -> Vec<usize> {
            let seps = divider_separators(&f, [y, y], VP, 24.);
            seps.iter().map(|s| s.id.group).collect()
        };
        assert_eq!(groups(10.), [0, 1]);
        assert_eq!(groups(20.), [1]);
        assert_eq!(groups(210.), [1, 2]);
        assert_eq!(groups(230.), [2]);
        assert!(divider_separators(&Folding::none(&d), [0., 0.], VP, 24.).is_empty());
    }

    #[test]
    fn stor_diff_gir_bare_synlige_band() {
        // 20k linjer med en endring hver 50. linje
        let a = lines("a", 20_000);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in (0..20_000).step_by(50) {
            b[i] = format!("x{i}");
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        assert_eq!(d.blocks().len(), 400);
        let y = 10_000. * VP.line_height;
        let bands = divider_bands(&d, &Folding::none(&d), [y, y], VP, 24.);
        assert_eq!(bands.iter().map(|b| b.block).collect::<Vec<_>>(), vec![200]);
    }
}
