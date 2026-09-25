//! Neste/forrige endring. Port av `PrevNextDifferenceIterableBase`,
//! `SimpleDiffViewer.scrollToChange` og `SyncScrollSupport.getTargetOffsets`. Ingen
//! animasjon: målet er sluttposisjonen `doMakeVisible` lander på.

use jetdiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::FileDiff;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavTarget {
    /// Caret flyttes til starten av blokken på begge sider.
    pub caret_lines: [usize; 2],
    pub scroll_y: [f32; 2],
    /// `doMakeVisible` scroller master til x = 0, og x deles.
    pub scroll_x: f32,
}

/// `canGoNext` og `goNext`: første blokk som starter under caret.
pub fn next_block(diff: &FileDiff, side: Side, caret_line: usize) -> Option<usize> {
    let blocks = diff.blocks();
    let last = blocks.last()?;
    if caret_line == diff.line_count(side) - 1 || last.lines(side).start <= caret_line {
        return None;
    }
    blocks.iter().position(|b| b.lines(side).start > caret_line)
}

/// `canGoPrev` og `goPrev`: siste blokk som ligger helt over caret. Står caret inne i en
/// blokk er det forrige blokk, ikke den man står i.
pub fn prev_block(diff: &FileDiff, side: Side, caret_line: usize) -> Option<usize> {
    let blocks = diff.blocks();
    let first = blocks.first()?;
    let first = first.lines(side);
    if caret_line == 0 || first.end > caret_line || first.start >= caret_line {
        return None;
    }
    (0..blocks.len()).find(|&i| match blocks.get(i + 1) {
        None => true,
        Some(next) => {
            let next = next.lines(side);
            next.end > caret_line || next.start >= caret_line
        }
    })
}

/// Endringer ligger aldri i en fold, og `scrollToChange` åpner ingenting.
pub fn target_for_block(
    diff: &FileDiff,
    folding: &Folding,
    block: usize,
    viewport: Viewport,
) -> NavTarget {
    let b = &diff.blocks()[block];
    let layouts = folding.layouts(viewport.line_height);
    let starts = [b.lines[0].start, b.lines[1].start];
    let ends = [b.lines[0].end, b.lines[1].end];
    NavTarget {
        caret_lines: starts,
        scroll_y: target_offsets(&layouts, starts, ends, viewport.height),
        scroll_x: 0.,
    }
}

/// `SyncScrollSupport.getTargetOffsets` med `preferredTopShift = -1`. Små endringer havner
/// 1/3 ned, samme punkt som sync-ankeret, så sync flytter ingenting etter hoppet.
fn target_offsets(
    layouts: &[LineLayout; 2],
    start_lines: [usize; 2],
    end_lines: [usize; 2],
    viewport_height: f32,
) -> [f32; 2] {
    let h = viewport_height;
    let mut top = [0.; 2];
    let mut range_h = [0.; 2];
    let mut gap = [0.; 2];
    let mut max = [0.; 2];
    let mut top_shifts = [0.; 2];
    for i in 0..2 {
        let l = &layouts[i];
        top[i] = l.line_top(start_lines[i]);
        // `endLine + 1` selv om endLine er eksklusiv, så det blir én linje ekstra luft
        range_h[i] = l.line_top(end_lines[i] + 1) - top[i];
        gap[i] = 2. * l.line_height();
        max[i] = l.max_scroll(h);
        let can_show = 2. * gap[i] + range_h[i] <= h;
        let shift = (h / 3.).floor();
        top_shifts[i] = if can_show {
            (h - gap[i] - range_h[i]).min(shift)
        } else {
            gap[i]
        };
    }

    // Toppen av fila
    let top_shift = top_shifts[0].min(top_shifts[1]).min(top[0].min(top[1]));
    let mut offsets = top.map(|t| t - top_shift);
    if (0..2).all(|i| max[i] > offsets[i]) {
        return clamp(offsets, max);
    }

    // En av sidene er ved slutten av fila: vis mest mulig, og alltid toppen av endringen
    let top_shift = (0..2).map(|i| top[i] - max[i]).fold(0f32, f32::max);
    for i in 0..2 {
        offsets[i] = top[i] - top_shift + (top_shift + range_h[i] + gap[i] - h).max(0.);
        offsets[i] = offsets[i].min(top[i] - gap[i]);
    }
    clamp(offsets, max)
}

/// Scrollbaren klemmer i IntelliJ.
fn clamp(offsets: [f32; 2], max: [f32; 2]) -> [f32; 2] {
    [offsets[0].clamp(0., max[0]), offsets[1].clamp(0., max[1])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    /// Endringer på linjene 10, 40 og 90 av 100, én linje hver.
    fn three_changes() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        FileDiff::new(&a, &(b.join("\n") + "\n"), ComparisonPolicy::Default).unwrap()
    }

    const VP: Viewport = Viewport {
        height: 300.,
        line_height: 20.,
    };

    #[test]
    fn navigasjon_over_folds() {
        let d = three_changes();
        let f = Folding::new(&d, crate::folding::FoldSettings::default());
        let mut seen = vec![0];
        let mut t = target_for_block(&d, &f, 0, VP);
        loop {
            let b = *seen.last().unwrap();
            let start = d.blocks()[b].lines[0].start;
            for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
                let layout = f.layout(side, 20.);
                // Endringen er synlig, og foldene over tar én linje hver
                let y = layout.line_top(start) - t.scroll_y[i];
                assert!((0. ..VP.height).contains(&y), "{t:?}");
                assert!(
                    layout.line_top(start) < Folding::none(&d).layout(side, 20.).line_top(start)
                );
            }
            let Some(next) = next_block(&d, Side::Right, t.caret_lines[1]) else {
                break;
            };
            seen.push(next);
            t = target_for_block(&d, &f, next, VP);
        }
        assert_eq!(seen, [0, 1, 2]);
        // Blokk 2 på linje 90 er visuell linje 25 med tre folds over
        assert_eq!(t.caret_lines, [90, 90]);
        assert_eq!(t.scroll_y, [400., 400.]);
        assert_eq!(prev_block(&d, Side::Right, t.caret_lines[1]), Some(1));
    }

    #[test]
    fn neste_og_forrige_fra_caret() {
        let d = three_changes();
        let s = Side::Right;
        assert_eq!(next_block(&d, s, 0), Some(0));
        assert_eq!(next_block(&d, s, 10), Some(1));
        assert_eq!(next_block(&d, s, 39), Some(1));
        assert_eq!(next_block(&d, s, 90), None);
        assert_eq!(next_block(&d, s, 100), None);

        assert_eq!(prev_block(&d, s, 0), None);
        assert_eq!(prev_block(&d, s, 10), None);
        assert_eq!(prev_block(&d, s, 11), Some(0));
        // Caret i blokk 1 gir blokk 0
        assert_eq!(prev_block(&d, s, 40), Some(0));
        assert_eq!(prev_block(&d, s, 41), Some(1));
        assert_eq!(prev_block(&d, s, 99), Some(2));
    }

    #[test]
    fn caret_pa_siste_linje_kan_ikke_ga_videre() {
        // Endring på siste linje med innhold, caret på den tomme linja etter
        let d = FileDiff::new("a\nb\n", "a\nc\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(next_block(&d, Side::Left, 0), Some(0));
        assert_eq!(next_block(&d, Side::Left, 2), None);
    }

    #[test]
    fn ingen_endringer() {
        let d = FileDiff::new("a\n", "a\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(next_block(&d, Side::Left, 0), None);
        assert_eq!(prev_block(&d, Side::Left, 1), None);
    }

    #[test]
    fn liten_endring_havner_en_tredjedel_ned() {
        let d = three_changes();
        let t = target_for_block(&d, &Folding::none(&d), 1, VP);
        assert_eq!(t.caret_lines, [40, 40]);
        // shift = min(300 - 40 - 40, 100) = 100
        assert_eq!(t.scroll_y, [700., 700.]);
    }

    #[test]
    fn endring_ved_toppen() {
        let d = three_changes();
        // top = 200 over shift 100, så shift blir 100
        assert_eq!(
            target_for_block(&d, &Folding::none(&d), 0, VP).scroll_y,
            [100., 100.]
        );
        let d = FileDiff::new("a\nb\n", "x\nb\n", ComparisonPolicy::Default).unwrap();
        assert_eq!(
            target_for_block(&d, &Folding::none(&d), 0, VP).scroll_y,
            [0., 0.]
        );
    }

    #[test]
    fn stor_endring_far_to_linjer_luft() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 20),
            lines("x", 30),
            &a[lines("a", 20).len()..]
        );
        let d = FileDiff::new(&a, &b, ComparisonPolicy::Default).unwrap();
        assert_eq!(d.blocks()[0].lines, [20..20, 20..50]);
        // Høyre kan ikke vises helt: shift = gap = 40 på begge
        assert_eq!(
            target_for_block(&d, &Folding::none(&d), 0, VP).scroll_y,
            [360., 360.]
        );
    }

    #[test]
    fn endring_ved_slutten_klemmes() {
        let d = three_changes();
        // top = 1800, max = (101 + 5) * 20 - 300 = 1820
        let t = target_for_block(&d, &Folding::none(&d), 2, VP);
        // Første forsøk 1700 < 1820, altså nok plass
        assert_eq!(t.scroll_y, [1700., 1700.]);

        let short = Viewport {
            height: 100.,
            line_height: 20.,
        };
        // Lavt vindu: shift = gap = 40 fordi 2*40 + 40 > 100
        assert_eq!(
            target_for_block(&d, &Folding::none(&d), 2, short).scroll_y,
            [1760., 1760.]
        );
    }

    #[test]
    fn fallback_nar_en_side_er_ved_slutten() {
        // Venstre kort, høyre lang: endringen ligger nederst på venstre
        let a = lines("a", 10) + "tail\n";
        let b = lines("a", 10) + "tail2\n" + &lines("z", 50);
        let d = FileDiff::new(&a, &b, ComparisonPolicy::Default).unwrap();
        let t = target_for_block(&d, &Folding::none(&d), 0, VP);
        let max_left = Folding::none(&d).layout(Side::Left, 20.).max_scroll(300.);
        assert_eq!(max_left, 40.);
        assert!(t.scroll_y[0] <= max_left);
        // Toppen av endringen er synlig på begge sider
        for (i, s) in [Side::Left, Side::Right].into_iter().enumerate() {
            let top = Folding::none(&d)
                .layout(s, 20.)
                .line_top(d.blocks()[0].lines(s).start);
            assert!(top >= t.scroll_y[i] && top < t.scroll_y[i] + 300.);
        }
    }
}
