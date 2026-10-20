//! Sync scroll. Port av `BaseSyncScrollable`, `SimpleDiffViewer.MySyncScrollable` og
//! `SyncScrollSupport.ScrollHelper.syncVerticalScroll`. Scroll-y er piksler fra toppen av
//! dokumentet, per side.

use jetdiff_core::side::Side;

use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::{Block, FileDiff, idx};

/// `Range` fra IntelliJ sett fra master: 1 er master, 2 er slave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SyncRange {
    start1: usize,
    end1: usize,
    start2: usize,
    end2: usize,
}

/// Grensepunktene `(0,0)`, start og slutt for hver blokk, og `(lineCount1, lineCount2)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMapping {
    bounds: Vec<[usize; 2]>,
    has_changes: bool,
}

impl LineMapping {
    pub fn new(blocks: &[Block], line_counts: [usize; 2]) -> Self {
        let mut bounds = vec![[0, 0]];
        for b in blocks {
            bounds.push([b.lines[0].start, b.lines[1].start]);
            bounds.push([b.lines[0].end, b.lines[1].end]);
        }
        bounds.push(line_counts);
        Self {
            bounds,
            has_changes: !blocks.is_empty(),
        }
    }

    /// `BaseSyncScrollable.getRange` med `ScrollHelper.process`: nabogrensene som omslutter linja.
    fn range(&self, master: Side, line: usize) -> SyncRange {
        if !self.has_changes {
            return id_range(line);
        }
        let (m, s) = (idx(master), idx(master.other()));
        // Første grense med `line <= b[m]`, ellers den siste. Binærsøk så scroll ikke blir
        // O(blokker) per frame.
        let i = self
            .bounds
            .partition_point(|b| b[m] < line)
            .min(self.bounds.len() - 1);
        let prev = if i == 0 { [0, 0] } else { self.bounds[i - 1] };
        let cur = self.bounds[i];
        SyncRange {
            start1: prev[m],
            end1: cur[m],
            start2: prev[s],
            end2: cur[s],
        }
    }

    pub fn transfer(&self, master: Side, line: usize) -> usize {
        transfer_line(line, self.range(master, line))
    }
}

/// `BaseSyncScrollable.transferLine`. Inne i en blokk går slave linje for linje fra toppen
/// og står stille når den korte siden er brukt opp.
fn transfer_line(line: usize, r: SyncRange) -> usize {
    if r.start1 == line {
        return r.start2;
    }
    if r.end1 == line {
        return r.end2;
    }
    if r.end1 < line {
        return line - r.end1 + r.end2;
    }
    (r.start2 + (line - r.start1)).min(r.end2)
}

fn id_range(line: usize) -> SyncRange {
    SyncRange {
        start1: line,
        end1: line + 1,
        start2: line,
        end2: line + 1,
    }
}

/// `ScrollHelper.transferVisualLine`: grensene konverteres til visuelle linjer i hver
/// editor, så foldede områder blir riktige uten spesialkode.
fn transfer_visual_line(
    mapping: &LineMapping,
    master: Side,
    master_layout: &LineLayout,
    slave_layout: &LineLayout,
    master_visual: usize,
) -> usize {
    let logical = master_layout.visual_to_logical(master_visual);
    let r = mapping.range(master, logical);
    let visual = |layout: &LineLayout, start: usize, end: usize| {
        let s = layout.logical_to_visual(start);
        let e = if start == end {
            s
        } else {
            layout.logical_to_visual(end)
        };
        (s, e)
    };
    let (start1, end1) = visual(master_layout, r.start1, r.end1);
    let (start2, end2) = visual(slave_layout, r.start2, r.end2);
    transfer_line(
        master_visual,
        SyncRange {
            start1,
            end1,
            start2,
            end2,
        },
    )
}

/// `ScrollHelper.syncVerticalScroll` og `doScrollVertically` uten anker og animasjon.
/// Ankeret ligger 1/3 ned i viewporten. Når slave står i en tom blokk flyttes den bare
/// ved hele linjer, så den ikke vibrerer mens master scroller gjennom en ren insert.
fn sync_vertical_scroll(
    mapping: &LineMapping,
    master: Side,
    master_layout: &LineLayout,
    slave_layout: &LineLayout,
    master_y: f32,
    slave_y: f32,
    viewport_height: f32,
) -> f32 {
    let lh = master_layout.line_height();
    let transfer =
        |visual| transfer_visual_line(mapping, master, master_layout, slave_layout, visual);

    // Heltallsdivisjon i Java
    let middle_y = (viewport_height / 3.).floor();
    let master_offset = master_y + middle_y;
    let master_visual = master_layout.y_to_visual_line(master_offset);
    let slave_visual = transfer(master_visual);

    let slave_offset = slave_layout.visual_line_to_y(slave_visual);
    let master_offset_raw = master_layout.visual_line_to_y(master_visual);
    // Holder ankerlinjene i samme fase
    let correction = (master_offset - master_offset_raw) % lh;
    let offset = slave_offset - middle_y + correction;

    let half = (lh / 2.).floor();
    let only_major_backward =
        correction < half && master_visual > 0 && slave_visual == transfer(master_visual - 1);
    let only_major_forward = correction > half && slave_visual == transfer(master_visual + 1);

    let target = if (only_major_forward && offset > slave_y
        || only_major_backward && offset < slave_y)
        && (offset - slave_y).abs() < slave_layout.line_height()
    {
        slave_y
    } else {
        offset
    };
    slave_layout.clamp_scroll(target, viewport_height)
}

/// Scroll for begge sider. Horisontal scroll deles, som i spiken og JetBrains
/// (`syncHorizontalScroll` setter samme x).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollState {
    pub y: [f32; 2],
    pub x: f32,
}

impl ScrollState {
    pub fn y(&self, side: Side) -> f32 {
        self.y[idx(side)]
    }

    /// Siden brukeren scroller er master. Slave regnes ut i samme frame.
    pub fn scroll_to(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        y: f32,
        viewport: Viewport,
    ) {
        let layout = folding.layout(master, viewport.line_height);
        let y = layout.clamp_scroll(y, viewport.height);
        // `visibleAreaChanged` synker bare når y faktisk endret seg
        if y == self.y(master) {
            return;
        }
        self.y[idx(master)] = y;
        self.sync_from(diff, folding, master, viewport);
    }

    pub fn scroll_by(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        dy: f32,
        viewport: Viewport,
    ) {
        self.scroll_to(diff, folding, master, self.y(master) + dy, viewport);
    }

    /// Regner slave på nytt, f.eks. etter endret viewport-høyde.
    pub fn sync_from(
        &mut self,
        diff: &FileDiff,
        folding: &Folding,
        master: Side,
        viewport: Viewport,
    ) {
        let slave = master.other();
        let master_layout = folding.layout(master, viewport.line_height);
        let slave_layout = folding.layout(slave, viewport.line_height);
        self.y[idx(slave)] = sync_vertical_scroll(
            diff.mapping(),
            master,
            &master_layout,
            &slave_layout,
            self.y(master),
            self.y(slave),
            viewport.height,
        );
    }

    /// `max_x` kommer fra UI-et, som kjenner tekstbredden.
    pub fn scroll_x_to(&mut self, x: f32, max_x: f32) {
        self.x = x.clamp(0., max_x.max(0.));
    }
}

/// `EditorScrollingPositionKeeper` per side: holder caret, eller øverste linje hvis caret er
/// utenfor, på samme sted på skjermen gjennom en fold-endring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionKeeper {
    /// Logisk linje og avstand fra toppen av viewporten.
    anchors: [(usize, f32); 2],
    caret: [bool; 2],
}

impl PositionKeeper {
    pub fn save(
        state: &ScrollState,
        folding: &Folding,
        carets: [usize; 2],
        viewport: Viewport,
    ) -> Self {
        let mut anchors = [(0, 0.); 2];
        let mut caret = [false; 2];
        for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let layout = folding.layout(side, viewport.line_height);
            let top = state.y[i];
            let caret_y = layout.line_top(carets[i]);
            let off_screen =
                caret_y + layout.line_height() <= top || caret_y >= top + viewport.height;
            anchors[i] = if off_screen {
                // `xyToLogicalPosition`: første linje i folden hvis toppen står på en
                let line = layout.y_to_logical_line(top).min(layout.line_count() - 1);
                (line, layout.line_top(line) - top)
            } else {
                (carets[i], caret_y - top)
            };
            caret[i] = !off_screen;
        }
        Self { anchors, caret }
    }

    /// `carets` er etter endringen, flyttet ut av kollapsede folds. Venstre gjenopprettes
    /// først uten sync, siden høyre fortsatt er i batch. Flytter høyre seg, synker den
    /// venstre som i `runBatchOperation`.
    pub fn restore(
        &self,
        state: &mut ScrollState,
        diff: &FileDiff,
        folding: &Folding,
        carets: [usize; 2],
        viewport: Viewport,
    ) {
        let before = state.y[1];
        for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let layout = folding.layout(side, viewport.line_height);
            let (line, shift) = self.anchors[i];
            let line = if self.caret[i] { carets[i] } else { line };
            state.y[i] = layout.clamp_scroll(layout.line_top(line) - shift, viewport.height);
        }
        if state.y[1] != before {
            state.sync_from(diff, folding, Side::Right, viewport);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;

    const L: Side = Side::Left;
    const R: Side = Side::Right;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    fn r(start1: usize, end1: usize, start2: usize, end2: usize) -> SyncRange {
        SyncRange {
            start1,
            end1,
            start2,
            end2,
        }
    }

    #[test]
    fn transfer_line_kanter() {
        let range = r(10, 20, 30, 33);
        assert_eq!(transfer_line(10, range), 30);
        assert_eq!(transfer_line(20, range), 33);
        assert_eq!(transfer_line(11, range), 31);
        assert_eq!(transfer_line(12, range), 32);
        // Klemt når den korte siden er brukt opp
        assert_eq!(transfer_line(13, range), 33);
        assert_eq!(transfer_line(19, range), 33);
        // Etter siste grense
        assert_eq!(transfer_line(25, range), 38);
    }

    #[test]
    fn transfer_line_tom_blokk() {
        // Ren insert på slave-siden: master står stille på grensen
        assert_eq!(transfer_line(5, r(5, 5, 5, 9)), 5);
        // Ren insert på master: hele blokken mappes til grensen
        let range = r(5, 9, 5, 5);
        for line in 5..=9 {
            assert_eq!(transfer_line(line, range), 5);
        }
    }

    fn mapping_for(a: &str, b: &str) -> (FileDiff, Vec<Block>) {
        let d = diff(a, b);
        let blocks = d.blocks().to_vec();
        (d, blocks)
    }

    #[test]
    fn transfer_for_og_etter_og_inni_blokk() {
        // venstre 0..10, høyre har 3 ekstra linjer etter linje 4
        let a = lines("a", 10);
        let b = format!(
            "{}{}{}",
            lines("a", 4),
            lines("x", 3),
            &lines("a", 10)[lines("a", 4).len()..]
        );
        let (d, blocks) = mapping_for(&a, &b);
        assert_eq!(blocks[0].lines, [4..4, 4..7]);
        let m = d.mapping();
        assert_eq!(m.transfer(L, 0), 0);
        assert_eq!(m.transfer(L, 3), 3);
        assert_eq!(m.transfer(L, 4), 4);
        assert_eq!(m.transfer(L, 5), 8);
        // Venstre har 11 linjer inkludert den tomme etter siste '\n'
        assert_eq!(m.transfer(L, 10), 13);
        // Forbi slutten, lineært
        assert_eq!(m.transfer(L, 14), 17);

        assert_eq!(m.transfer(R, 4), 4);
        assert_eq!(m.transfer(R, 5), 4);
        assert_eq!(m.transfer(R, 6), 4);
        assert_eq!(m.transfer(R, 7), 4);
        assert_eq!(m.transfer(R, 8), 5);
    }

    #[test]
    fn transfer_start_og_slutt_av_dokument() {
        // Insert helt i starten og slutten
        let a = "m\n";
        let b = "x\ny\nm\nz\n";
        let d = diff(a, b);
        let m = d.mapping();
        assert_eq!(d.blocks()[0].lines, [0..0, 0..2]);
        assert_eq!(m.transfer(L, 0), 0);
        assert_eq!(m.transfer(R, 0), 0);
        assert_eq!(m.transfer(R, 1), 0);
        assert_eq!(m.transfer(R, 2), 0);
        assert_eq!(m.transfer(L, 1), d.blocks()[1].lines[1].start);
        assert_eq!(m.transfer(L, 2), 5);
        assert_eq!(m.transfer(R, 5), 2);
    }

    #[test]
    fn binaersok_gir_samme_range_som_lineaert_sok() {
        let a = lines("a", 60);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        b[3] = "x".into();
        b.insert(20, "y".into());
        b.insert(20, "z".into());
        b.remove(40);
        b.truncate(55);
        let d = diff(&a, &(b.join("\n") + "\n"));
        let m = d.mapping();
        for master in [L, R] {
            let (mi, si) = (idx(master), idx(master.other()));
            for line in 0..70 {
                let (mut prev, mut cur) = ([0, 0], [0, 0]);
                for &bound in &m.bounds {
                    prev = cur;
                    cur = bound;
                    if line <= bound[mi] {
                        break;
                    }
                }
                let expected = r(prev[mi], cur[mi], prev[si], cur[si]);
                assert_eq!(m.range(master, line), expected, "{master:?} {line}");
            }
        }
    }

    #[test]
    fn uten_endringer_er_mapping_identitet() {
        let d = diff("a\nb\n", "a\nb\n");
        assert_eq!(d.mapping().transfer(L, 1), 1);
        assert_eq!(d.mapping().transfer(R, 40), 40);
    }

    const VP: Viewport = Viewport {
        height: 300.,
        line_height: 20.,
    };

    fn sync(d: &FileDiff, master: Side, master_y: f32, slave_y: f32) -> f32 {
        sync_folded(d, &Folding::none(d), master, master_y, slave_y)
    }

    fn sync_folded(d: &FileDiff, f: &Folding, master: Side, master_y: f32, slave_y: f32) -> f32 {
        let (ml, sl) = (f.layout(master, 20.), f.layout(master.other(), 20.));
        sync_vertical_scroll(d.mapping(), master, &ml, &sl, master_y, slave_y, VP.height)
    }

    /// Endringer på linjene 10, 40 og 90 av 100. Kollapset: folds 0..6, 15..36, 45..86 og
    /// 95..101 på begge sider.
    fn three_changes() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        diff(&a, &(b.join("\n") + "\n"))
    }

    fn folded(d: &FileDiff) -> Folding {
        Folding::new(d)
    }

    #[test]
    fn expand_holder_caret_pa_plass() {
        let d = three_changes();
        let mut f = folded(&d);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 400., VP);
        assert_eq!(s.y, [400., 400.]);
        let carets = [90, 90];
        let screen = |f: &Folding, s: &ScrollState| {
            [L, R].map(|side| f.layout(side, 20.).line_top(90) - s.y(side))
        };
        let before = screen(&f, &s);
        let keeper = PositionKeeper::save(&s, &f, carets, VP);
        let id = f.fold_at_line(L, 45).unwrap();
        f.set_expanded(id, true);
        keeper.restore(&mut s, &d, &f, carets, VP);
        // 45..86 ble 4 + fold + 4 linjer, altså 8 visuelle linjer mer over caret
        assert_eq!(s.y, [560., 560.]);
        assert_eq!(screen(&f, &s), before);
    }

    #[test]
    fn caret_utenfor_holder_overste_linje() {
        let d = three_changes();
        let mut f = folded(&d);
        let mut s = ScrollState::default();
        // Visuell linje 11 er logisk 36
        s.scroll_to(&d, &f, L, 220., VP);
        let carets = [0, 0];
        let keeper = PositionKeeper::save(&s, &f, carets, VP);
        // 0..6 blir fold 0..2 og fire linjer, altså fire visuelle linjer mer
        let id = f.fold_at_line(R, 0).unwrap();
        f.set_expanded(id, true);
        keeper.restore(&mut s, &d, &f, carets, VP);
        assert_eq!(s.y, [300., 300.]);
        assert_eq!(f.layout(L, 20.).y_to_logical_line(s.y[0]), 36);

        // Toppen står på en fold som åpnes: første linje blir liggende
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 480., VP);
        assert_eq!(f.layout(L, 20.).y_to_logical_line(480.), 45);
        let keeper = PositionKeeper::save(&s, &f, carets, VP);
        f.expand_at(L, 45);
        keeper.restore(&mut s, &d, &f, carets, VP);
        assert_eq!(s.y, [480., 480.]);
    }

    #[test]
    fn collapse_flytter_caret_til_folden() {
        let d = three_changes();
        let mut f = folded(&d);
        f.expand_all(true);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &f, L, 1000., VP);
        let carets = [60, 60];
        let keeper = PositionKeeper::save(&s, &f, carets, VP);
        f.expand_all(false);
        let carets = carets.map(|c| f.caret_after(L, c));
        assert_eq!(carets, [45, 45]);
        keeper.restore(&mut s, &d, &f, carets, VP);
        // Folden havner der caret-linja sto
        assert_eq!(f.layout(L, 20.).line_top(45) - s.y[0], 1200. - 1000.);
    }

    #[test]
    fn sync_med_ulike_folds() {
        // Høyre har 30 ekstra linjer ved 40, så gapene og foldene er ulike
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        b[10] = "x".into();
        b[90] = "y".into();
        for i in 0..30 {
            b.insert(40, format!("n{i}"));
        }
        let d = diff(&a, &(b.join("\n") + "\n"));
        let f = folded(&d);
        let (ml, sl) = (f.layout(L, 20.), f.layout(R, 20.));
        assert_ne!(ml.folds(), sl.folds());
        let anchor = (VP.height / 3.).floor();
        let mut checked = 0;
        let mut y = 0.;
        while y <= ml.max_scroll(VP.height) {
            let slave = sync_folded(&d, &f, L, y, 0.);
            let master_line = ml.y_to_logical_line(y + anchor);
            let in_block = d.blocks().iter().any(|b| b.lines[0].contains(&master_line));
            let in_fold = ml
                .folds()
                .iter()
                .any(|fold| fold.lines.contains(&master_line));
            let clamped = slave == 0. || slave == sl.max_scroll(VP.height);
            if !in_block && !in_fold && !clamped {
                let slave_line = sl.y_to_logical_line(slave + anchor);
                assert_eq!(slave_line, d.mapping().transfer(L, master_line), "y={y}");
                checked += 1;
            }
            y += 5.;
        }
        assert!(checked > 20);
    }

    #[test]
    fn like_filer_scroller_likt() {
        let a = lines("a", 100);
        let d = diff(&a, &a);
        for y in [0., 7., 20., 333.5, 1000.] {
            assert_eq!(sync(&d, L, y, 0.), y);
        }
    }

    #[test]
    fn anker_en_tredjedel_ned() {
        // 10 ekstra linjer på høyre side etter linje 50
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        assert_eq!(d.blocks()[0].lines, [50..50, 50..60]);
        // Anker (y+100) over blokken: likt
        assert_eq!(sync(&d, L, 800., 800.), 800.);
        // Anker under blokken: 10 linjer forskjøvet
        assert_eq!(sync(&d, L, 1100., 0.), 1300.);
        // Fra høyre: ankeret inni den innsatte blokken treffer grensen på venstre side
        assert_eq!(sync(&d, R, 950., 0.), 50. * 20. - 100. + 50. % 20.);
    }

    #[test]
    fn slave_flytter_bare_hele_linjer_i_tom_blokk() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        // Master = høyre, ankeret (y + 100) går gjennom de innsatte linjene 50..60.
        // Slave (venstre) følger fasen til midten av første linje og står så stille
        // til ankeret er halvveis i linja etter blokken.
        let mut slave = 0.;
        for y in 880..1140 {
            let y = y as f32;
            let prev = slave;
            slave = sync(&d, R, y, slave);
            assert!(slave >= prev, "slave gikk bakover ved master {y}");
            let expected = match y {
                ..=910. => y,
                ..1110. => 910.,
                _ => y - 200.,
            };
            assert_eq!(slave, expected, "master {y}");
        }
    }

    #[test]
    fn slave_klemmes() {
        // Venstre kort, høyre lang: slave kan ikke gå forbi sin maks
        let a = lines("a", 5);
        let b = lines("a", 200);
        let d = diff(&a, &b);
        let right_max = Folding::none(&d).layout(R, 20.).max_scroll(VP.height);
        assert_eq!(
            sync(&d, R, right_max, 0.),
            Folding::none(&d).layout(L, 20.).max_scroll(VP.height)
        );
        assert_eq!(sync(&d, L, 0., 500.), 0.);
    }

    #[test]
    fn scroll_state_bytter_master() {
        let a = lines("a", 100);
        let b = format!(
            "{}{}{}",
            lines("a", 50),
            lines("x", 10),
            &a[lines("a", 50).len()..]
        );
        let d = diff(&a, &b);
        let mut s = ScrollState::default();
        s.scroll_to(&d, &Folding::none(&d), L, 1100., VP);
        assert_eq!(s.y, [1100., 1300.]);
        s.scroll_by(&d, &Folding::none(&d), R, -1300., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, &Folding::none(&d), L, -50., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, &Folding::none(&d), L, 1e9, VP);
        assert_eq!(
            s.y(L),
            Folding::none(&d).layout(L, 20.).max_scroll(VP.height)
        );
        s.scroll_x_to(-4., 100.);
        assert_eq!(s.x, 0.);
        s.scroll_x_to(400., 100.);
        assert_eq!(s.x, 100.);
    }
}
