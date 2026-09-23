//! Visningsmodellen mot fixtures fra `fixtures/`, med kjente tall for unicode og manglende
//! linjeskift, og invarianter over alle.

use std::fs;
use std::path::{Path, PathBuf};

use smudiff_view::{
    ComparisonPolicy, DiffType, FileDiff, InlineHighlight, Navigation, ScrollState, Side, Viewport,
    divider_bands, stripe_markers,
};

const L: Side = Side::Left;
const R: Side = Side::Right;

const VP: Viewport = Viewport {
    height: 400.,
    line_height: 18.,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load(name: &str) -> FileDiff {
    let dir = root().join(name);
    let a = fs::read_to_string(dir.join("a.txt")).unwrap();
    let b = fs::read_to_string(dir.join("b.txt")).unwrap();
    FileDiff::new(&a, &b, ComparisonPolicy::Default).unwrap()
}

fn all() -> Vec<(String, FileDiff)> {
    let mut names: Vec<String> = fs::read_dir(root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("a.txt").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.into_iter().map(|n| (n.clone(), load(&n))).collect()
}

fn h(cols: std::ops::Range<usize>, diff_type: DiffType) -> InlineHighlight {
    InlineHighlight {
        cols,
        diff_type,
        past_line_end: false,
    }
}

#[test]
fn norsk_tekst_gir_byte_kolonner() {
    let d = load("unicode-norwegian");
    let left0 = d.line_highlight(L, 0);
    let bg = left0.background.unwrap();
    assert_eq!(bg.diff_type, DiffType::Modified);
    assert!(bg.ignored);
    assert_eq!(left0.inline, [h(22..32, DiffType::Modified)]);
    assert_eq!(&d.line(L, 0)[22..32], "brødskiva");
    let left1 = d.line_highlight(L, 1);
    assert_eq!(left1.inline, [h(20..24, DiffType::Modified)]);
    assert_eq!(&d.line(L, 1)[20..24], "godt");
    assert_eq!(d.line_highlight(R, 0).inline[0].cols, 22..33);
}

#[test]
fn emoji_med_tomme_markorer() {
    let d = load("unicode-emoji");
    assert_eq!(
        d.line_highlight(L, 0).inline,
        [h(8..12, DiffType::Modified)]
    );
    assert_eq!(&d.line(L, 0)[8..12], "🚀");
    // Tomme sider av ord-fragmentene blir 2px-streker
    assert_eq!(
        d.line_highlight(L, 1).inline,
        [h(8..8, DiffType::Inserted), h(11..11, DiffType::Inserted)]
    );
    assert_eq!(
        d.line_highlight(R, 1).inline,
        [h(8..12, DiffType::Inserted), h(15..20, DiffType::Inserted)]
    );
    assert_eq!(&d.line(R, 1)[15..20], " done");
    assert_eq!(d.line_highlight(L, 2).background, None);
}

#[test]
fn manglende_linjeskift_til_venstre() {
    let d = load("missing-trailing-newline-left");
    assert_eq!(d.line_count(L), 3);
    assert_eq!(d.line_count(R), 4);
    let b = &d.blocks()[0];
    assert_eq!(b.lines, [3..3, 3..4]);
    assert_eq!(b.diff_type, DiffType::Inserted);
    // `Some(vec![])`: ignored-farge, ingen ord
    assert_eq!(b.inner, Some(vec![]));
    let right = d.line_highlight(R, 3);
    assert!(right.background.unwrap().ignored);
    assert!(right.inline.is_empty());
    // Blokken ligger etter siste venstre linje, så 2px-båndet går under linje 2
    let layout = d.layout(L, 18.);
    assert_eq!(layout.block_band(b.lines(L)), (53., 55.));
    let band = divider_bands(&d, [0., 0.], VP, 24.)[0];
    assert_eq!((band.top.p0.y, band.bottom.p0.y), (53., 55.));
    assert_eq!((band.top.p3.y, band.bottom.p3.y), (54., 72.));
}

#[test]
fn manglende_linjeskift_til_hoyre() {
    let d = load("missing-trailing-newline-right");
    let b = &d.blocks()[0];
    assert_eq!(b.lines, [3..4, 3..3]);
    assert_eq!(b.diff_type, DiffType::Deleted);
}

#[test]
fn manglende_linjeskift_og_endring() {
    let d = load("missing-trailing-newline-both-changed");
    assert_eq!(d.blocks()[0].lines, [2..3, 2..3]);
    let left = d.line_highlight(L, 2);
    assert_eq!(left.inline, [h(9..9, DiffType::Inserted)]);
    let right = d.line_highlight(R, 2);
    // Siste linje uten linjeskift, så fargen stopper ved teksten
    assert_eq!(right.inline, [h(9..d.line(R, 2).len(), DiffType::Inserted)]);
    assert_eq!(d.line(R, 2).len(), 17);
}

#[test]
fn highlights_ligger_pa_tegngrenser() {
    for (name, d) in all() {
        for side in [L, R] {
            let blocks_lines: Vec<_> = d.blocks().iter().map(|b| b.lines(side)).collect();
            for line in 0..d.line_count(side) {
                let text = d.line(side, line);
                let hl = d.line_highlight(side, line);
                let in_block = blocks_lines.iter().any(|r| r.contains(&line));
                assert_eq!(hl.background.is_some(), in_block, "{name} {side:?} {line}");
                if hl.background.is_some_and(|b| !b.ignored) {
                    assert!(hl.inline.is_empty(), "{name}: hel-linje har ikke ord");
                }
                for i in hl.inline {
                    assert!(i.cols.end <= text.len(), "{name} {side:?} {line} {i:?}");
                    assert!(text.is_char_boundary(i.cols.start), "{name} {i:?}");
                    assert!(text.is_char_boundary(i.cols.end), "{name} {i:?}");
                    if i.past_line_end {
                        assert!(line + 1 < d.line_count(side), "{name} {i:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn scroll_og_divider_holder_seg_innenfor() {
    for (name, d) in all() {
        let mut s = ScrollState::default();
        let max = [
            d.layout(L, 18.).max_scroll(VP.height),
            d.layout(R, 18.).max_scroll(VP.height),
        ];
        let mut y = 0.;
        while y <= max[0] + 50. {
            s.scroll_to(&d, L, y, VP);
            assert!(s.y[1] >= 0. && s.y[1] <= max[1], "{name} {s:?}");
            for band in divider_bands(&d, s.y, VP, 24.) {
                assert!(band.top.p0.y < band.bottom.p0.y, "{name}");
                assert!(band.top.p3.y < band.bottom.p3.y, "{name}");
                // Lukket intervall: en blokk som slutter på første malte linje er med
                let min = -VP.line_height - 1.;
                assert!(band.bottom.p0.y >= min || band.bottom.p3.y >= min, "{name}");
            }
            y += 7.;
        }
    }
}

#[test]
fn navigasjon_besoker_alle_blokker() {
    for (name, d) in all() {
        if d.blocks().is_empty() {
            continue;
        }
        // Startposisjon er første endring, som i `InitialScrollPositionSupport`
        let mut nav = Navigation::default();
        let mut seen = vec![nav.go_to(&d, 0, VP).block];
        while let Some(t) = nav.go_next(&d, VP) {
            seen.push(t.block);
            for (i, side) in [L, R].into_iter().enumerate() {
                assert!(t.scroll_y[i] >= 0., "{name}");
                assert!(
                    t.scroll_y[i] <= d.layout(side, 18.).max_scroll(VP.height),
                    "{name}"
                );
            }
        }
        let expected: Vec<usize> = (0..d.blocks().len()).collect();
        assert_eq!(seen, expected, "{name}");
        // Og tilbake igjen
        let mut back = vec![];
        while let Some(t) = nav.go_prev(&d, VP) {
            back.push(t.block);
        }
        back.reverse();
        assert_eq!(back, expected[..expected.len() - 1], "{name}");
    }
}

#[test]
fn stripe_per_side() {
    let d = load("insert-middle");
    let left = stripe_markers(&d, L, 18.);
    let right = stripe_markers(&d, R, 18.);
    assert_eq!(left.len(), d.blocks().len());
    assert_eq!(left[0].diff_type, DiffType::Inserted);
    assert!(left[0].start == left[0].end);
    assert!(right[0].start <= right[0].end);
    assert!(right[0].end < 1.);
}
