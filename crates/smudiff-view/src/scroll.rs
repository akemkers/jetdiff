//! Sync scroll. Port av `BaseSyncScrollable`, `SimpleDiffViewer.MySyncScrollable` og
//! `SyncScrollSupport.ScrollHelper.syncVerticalScroll`. Scroll-y er piksler fra toppen av
//! dokumentet, per side.

use smudiff_core::side::Side;

use crate::layout::{LineLayout, Viewport};
use crate::model::{Block, FileDiff, idx};

/// `Range` fra IntelliJ sett fra master: 1 er master, 2 er slave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncRange {
    pub start1: usize,
    pub end1: usize,
    pub start2: usize,
    pub end2: usize,
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
    pub fn range(&self, master: Side, line: usize) -> SyncRange {
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
pub fn transfer_line(line: usize, r: SyncRange) -> usize {
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
pub fn transfer_visual_line(
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
#[allow(clippy::too_many_arguments)]
pub fn sync_vertical_scroll(
    mapping: &LineMapping,
    master: Side,
    master_layout: &LineLayout,
    slave_layout: &LineLayout,
    master_empty: bool,
    master_y: f32,
    slave_y: f32,
    viewport_height: f32,
) -> f32 {
    // `master.getDocument().getTextLength() == 0`: ingen sync
    if master_empty {
        return slave_y;
    }
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
    pub fn scroll_to(&mut self, diff: &FileDiff, master: Side, y: f32, viewport: Viewport) {
        let layout = diff.layout(master, viewport.line_height);
        let y = layout.clamp_scroll(y, viewport.height);
        // `visibleAreaChanged` synker bare når y faktisk endret seg
        if y == self.y(master) {
            return;
        }
        self.y[idx(master)] = y;
        self.sync_from(diff, master, viewport);
    }

    pub fn scroll_by(&mut self, diff: &FileDiff, master: Side, dy: f32, viewport: Viewport) {
        self.scroll_to(diff, master, self.y(master) + dy, viewport);
    }

    /// Regner slave på nytt, f.eks. etter endret viewport-høyde.
    pub fn sync_from(&mut self, diff: &FileDiff, master: Side, viewport: Viewport) {
        let slave = master.other();
        let master_layout = diff.layout(master, viewport.line_height);
        let slave_layout = diff.layout(slave, viewport.line_height);
        self.y[idx(slave)] = sync_vertical_scroll(
            diff.mapping(),
            master,
            &master_layout,
            &slave_layout,
            diff.text(master).is_empty(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use smudiff_core::ComparisonPolicy;

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
        let (ml, sl) = (d.layout(master, 20.), d.layout(master.other(), 20.));
        sync_vertical_scroll(
            d.mapping(),
            master,
            &ml,
            &sl,
            d.text(master).is_empty(),
            master_y,
            slave_y,
            VP.height,
        )
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
        let right_max = d.layout(R, 20.).max_scroll(VP.height);
        assert_eq!(
            sync(&d, R, right_max, 0.),
            d.layout(L, 20.).max_scroll(VP.height)
        );
        assert_eq!(sync(&d, L, 0., 500.), 0.);
    }

    #[test]
    fn tom_master_synker_ikke() {
        let d = diff("", &lines("a", 50));
        assert_eq!(sync(&d, L, 0., 123.), 123.);
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
        s.scroll_to(&d, L, 1100., VP);
        assert_eq!(s.y, [1100., 1300.]);
        s.scroll_by(&d, R, -1300., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, L, -50., VP);
        assert_eq!(s.y, [0., 0.]);
        s.scroll_to(&d, L, 1e9, VP);
        assert_eq!(s.y(L), d.layout(L, 20.).max_scroll(VP.height));
        s.scroll_x_to(-4., 100.);
        assert_eq!(s.x, 0.);
        s.scroll_x_to(400., 100.);
        assert_eq!(s.x, 100.);
    }
}
