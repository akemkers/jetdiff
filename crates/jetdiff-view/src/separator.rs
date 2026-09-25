//! Bølgen på kollapsede folds og koblingen i divideren. Port av `DiffLineSeparatorRenderer`.
//! Stier er rene data som UI-laget streker med `stroke_width`.

use std::ops::Range;

use crate::divider::{DIVIDER_WIDTH, Point};

/// `getStepSize`: kvart periode. Divideren trenger 6 steg for å se pen ut.
const STEP: f32 = DIVIDER_WIDTH / 6.;
/// `getHeight`: halv høyde på bølgen.
const HEIGHT: f32 = 3.;
/// `getAAGap`.
const AA_GAP: f32 = 1.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSeg {
    Move(Point),
    Quad { ctrl: Point, to: Point },
    Cubic { c1: Point, c2: Point, to: Point },
}

/// `getStroke`.
pub fn stroke_width(hovered: bool) -> f32 {
    if hovered { 2. } else { 1. }
}

/// `getVerticalOffset`, i heltall som i Java.
fn vertical_offset(line_height: f32) -> f32 {
    ((line_height - 2. * HEIGHT - 2. * AA_GAP) / 2.).floor()
}

/// Kvartene i bølgebildet fra `createImage`, som (start, kontroll, slutt) i y.
const QUARTERS: [(f32, f32, f32); 4] = {
    let (upper, center, lower) = (AA_GAP, HEIGHT + AA_GAP, 2. * HEIGHT + AA_GAP);
    [
        (upper, upper, center),
        (center, lower, lower),
        (lower, lower, center),
        (center, upper, upper),
    ]
};

/// Bølge over `x` på linja som starter på `top`. `edge` er kanten mot divideren. Fasen
/// regnes fra den, så bølgen møter koblingen midt på (`getStartPhase`) og står stille ved
/// horisontal scroll. Speilet på venstre side, men bildet er symmetrisk.
pub fn wave(x: Range<f32>, edge: f32, mirrored: bool, top: f32, line_height: f32) -> Vec<PathSeg> {
    let sign = if mirrored { -1. } else { 1. };
    let (d0, d1) = {
        let (a, b) = ((x.start - edge) * sign, (x.end - edge) * sign);
        (a.min(b), a.max(b))
    };
    let y0 = top + vertical_offset(line_height);
    let at = |d: f32, y: f32| Point {
        x: edge + sign * d,
        y: y0 + y,
    };
    let (k0, k1) = ((d0 / STEP).floor() as i64, (d1 / STEP).ceil() as i64);
    let mut path = Vec::with_capacity((k1 - k0) as usize + 1);
    for k in k0..k1 {
        // Ved kanten er vi midt i fallende kvart: `(d + 3 * STEP) mod 4 * STEP`
        let (start, ctrl, end) = QUARTERS[(k + 3).rem_euclid(4) as usize];
        let d = k as f32 * STEP;
        if k == k0 {
            path.push(PathSeg::Move(at(d, start)));
        }
        path.push(PathSeg::Quad {
            ctrl: at(d + STEP / 2., ctrl),
            to: at(d + STEP, end),
        });
    }
    path
}

/// `drawConnectorLine` fra venstre fold (y1) til høyre (y2). y er toppen av linja, lokalt
/// for divideren.
pub fn connector(width: f32, y1: f32, y2: f32, line_height: f32) -> Vec<PathSeg> {
    let (x1, x2) = (0., width);
    let (s, h) = (STEP, HEIGHT);
    let align = vertical_offset(line_height) + h + AA_GAP;
    let (y1, y2) = (y1 + align, y2 + align);
    let p = |x, y| Point { x, y };
    let quad = |cx, cy, x, y| PathSeg::Quad {
        ctrl: p(cx, cy),
        to: p(x, y),
    };
    let cubic = |ax, ay, bx, by, x, y| PathSeg::Cubic {
        c1: p(ax, ay),
        c2: p(bx, by),
        to: p(x, y),
    };
    // 0.5: avrunding i speilet modus
    let mut path = vec![PathSeg::Move(p(x1 - 0.5, y1))];
    let delta = (y2 - y1).abs() / (x2 - x1).abs();
    if delta < 0.2 {
        let (mx, my) = ((x1 + x2) / 2., (y1 + y2) / 2.);
        if x2 - x1 > 5. * s {
            path.extend([
                quad(x1 + s * 0.5, y1 + h, x1 + s, y1 + h),
                quad(x1 + s * 1.5, y1 + h, x1 + s * 2., my),
                quad(x1 + s * 2.5, my - h, mx, my - h),
                quad(x2 - s * 2.5, my - h, x2 - s * 2., my),
                quad(x2 - s * 1.5, y2 + h, x2 - s, y2 + h),
                quad(x2 - s * 0.5, y2 + h, x2, y2),
            ]);
        } else {
            // For smal divider: to halvperioder i stedet for seks
            path.push(quad(mx, my + 2. * h, x2, y2));
        }
    } else if y1 > y2 {
        path.extend([
            cubic(
                x1 + s * 0.125,
                y1 + h * 0.125,
                x1 + s * 0.125,
                y1 + h * 0.5,
                x1 + s * 0.5,
                y1 + h * 0.5,
            ),
            cubic(x2 - s * 2., y1 + h * 0.5, x2 - s * 2., y2 + 4. * h, x2, y2),
        ]);
    } else {
        path.extend([
            cubic(
                x1 + s * 2.,
                y1 + 4. * h,
                x1 + s * 2.,
                y2 + h * 0.5,
                x2 - s * 0.5,
                y2 + h * 0.5,
            ),
            cubic(
                x2 - s * 0.125,
                y2 + h * 0.5,
                x2 - s * 0.125,
                y2 + h * 0.125,
                x2,
                y2,
            ),
        ]);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(seg: &PathSeg) -> Point {
        match *seg {
            PathSeg::Move(p) => p,
            PathSeg::Quad { to, .. } | PathSeg::Cubic { to, .. } => to,
        }
    }

    #[test]
    fn parametere_for_standard_divider() {
        assert_eq!(STEP, 4.);
        // Linjehøyde 18 gir bølgen i y 5..13
        assert_eq!(vertical_offset(18.), 5.);
        assert_eq!(vertical_offset(17.), 4.);
    }

    #[test]
    fn bolgen_har_periode_16_og_3px_utslag() {
        let path = wave(100. ..132., 100., false, 0., 18.);
        assert_eq!(path.len(), 9);
        assert_eq!(path[0], PathSeg::Move(Point { x: 100., y: 9. }));
        let ys: Vec<f32> = path.iter().map(|s| end(s).y).collect();
        // Midt, opp, midt, ned, midt, opp ...
        assert_eq!(ys, [9., 6., 9., 12., 9., 6., 9., 12., 9.]);
        assert!(ys.iter().all(|y| (y - 9.).abs() <= HEIGHT));
    }

    #[test]
    fn venstre_side_er_speilet_om_divideren() {
        let right = wave(100. ..120., 100., false, 0., 18.);
        let left = wave(80. ..100., 100., true, 0., 18.);
        assert_eq!(left.len(), right.len());
        for (l, r) in left.iter().zip(&right) {
            let (l, r) = (end(l), end(r));
            assert_eq!(l.y, r.y);
            assert_eq!(100. - l.x, r.x - 100.);
        }
    }

    #[test]
    fn fasen_folger_kanten_ikke_starten() {
        // Starter midt i en periode: samme y som bølgen fra kanten på samme x
        let whole = wave(100. ..140., 100., false, 0., 18.);
        let part = wave(109. ..140., 100., false, 0., 18.);
        let at = |path: &[PathSeg], x: f32| path.iter().map(end).find(|p| p.x == x).unwrap().y;
        for x in [112., 116., 120., 136.] {
            assert_eq!(at(&whole, x), at(&part, x));
        }
        assert_eq!(end(&part[0]).x, 108.);
    }

    #[test]
    fn kobling_mot_midten_av_bolgen() {
        // Rett kobling, seks halvperioder
        let path = connector(24., 10., 10., 18.);
        assert_eq!(path.len(), 7);
        assert_eq!(path[0], PathSeg::Move(Point { x: -0.5, y: 19. }));
        assert_eq!(end(&path[6]), Point { x: 24., y: 19. });
        // Samme y som bølgen ved kanten
        assert_eq!(end(&wave(0. ..4., 0., false, 10., 18.)[0]).y, 19.);
        assert_eq!(
            path[3],
            PathSeg::Quad {
                ctrl: Point { x: 10., y: 16. },
                to: Point { x: 12., y: 16. },
            }
        );
    }

    #[test]
    fn bratt_kobling_er_to_kurver() {
        let down = connector(24., 0., 100., 18.);
        assert_eq!(down.len(), 3);
        assert!(matches!(down[1], PathSeg::Cubic { .. }));
        assert_eq!(end(&down[2]), Point { x: 24., y: 109. });
        let up = connector(24., 100., 0., 18.);
        assert_eq!(end(&up[1]), Point { x: 2., y: 110.5 });
        // Smal divider faller tilbake til én kvadratisk
        assert_eq!(connector(10., 0., 1., 18.).len(), 2);
    }
}
