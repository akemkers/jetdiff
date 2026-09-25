//! Visningsmodellen mot fixtures fra `fixtures/`, med kjente tall for unicode og manglende
//! linjeskift, og invarianter over alle.

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use smudiff_view::folding::CONTEXT_RANGE_MODES;
use smudiff_view::nav::{next_block, prev_block, target_for_block};
use smudiff_view::{
    Command, ComparisonPolicy, DiffState, DiffType, FileDiff, FoldSettings, Folding, Horizontal,
    InlineHighlight, PositionKeeper, ScrollState, Side, Viewport, divider_bands,
    divider_separators, stripe_markers,
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
    let block = &d.blocks()[left0.block.unwrap()];
    assert_eq!(block.diff_type, DiffType::Modified);
    assert!(block.ignored());
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
    assert_eq!(d.line_highlight(L, 2).block, None);
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
    assert!(d.blocks()[right.block.unwrap()].ignored());
    assert!(right.inline.is_empty());
    // Blokken ligger etter siste venstre linje, så 2px-båndet går under linje 2
    let none = Folding::none(&d);
    let layout = none.layout(L, 18.);
    assert_eq!(layout.block_band(b.lines(L)), (53., 55.));
    let band = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
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
                assert_eq!(hl.block.is_some(), in_block, "{name} {side:?} {line}");
                if hl.block.is_some_and(|b| !d.blocks()[b].ignored()) {
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
            Folding::none(&d).layout(L, 18.).max_scroll(VP.height),
            Folding::none(&d).layout(R, 18.).max_scroll(VP.height),
        ];
        let mut y = 0.;
        while y <= max[0] + 50. {
            s.scroll_to(&d, &Folding::none(&d), L, y, VP);
            assert!(s.y[1] >= 0. && s.y[1] <= max[1], "{name} {s:?}");
            for band in divider_bands(&d, &Folding::none(&d), s.y, VP, 24.) {
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
        let f = Folding::none(&d);
        // Startposisjon er første endring, som i `InitialScrollPositionSupport`
        let mut seen = vec![0];
        let mut caret = target_for_block(&d, &f, 0, VP).caret_lines[1];
        while let Some(block) = next_block(&d, R, caret) {
            seen.push(block);
            let t = target_for_block(&d, &f, block, VP);
            caret = t.caret_lines[1];
            for (i, side) in [L, R].into_iter().enumerate() {
                assert!(t.scroll_y[i] >= 0., "{name}");
                assert!(
                    t.scroll_y[i] <= Folding::none(&d).layout(side, 18.).max_scroll(VP.height),
                    "{name}"
                );
            }
        }
        let expected: Vec<usize> = (0..d.blocks().len()).collect();
        assert_eq!(seen, expected, "{name}");
        // Og tilbake igjen
        let mut back = vec![];
        while let Some(block) = prev_block(&d, R, caret) {
            back.push(block);
            caret = target_for_block(&d, &f, block, VP).caret_lines[1];
        }
        back.reverse();
        assert_eq!(back, expected[..expected.len() - 1], "{name}");
    }
}

#[test]
fn stripe_per_side() {
    let d = load("insert-middle");
    let left = stripe_markers(&d, &Folding::none(&d), L, 18.);
    let right = stripe_markers(&d, &Folding::none(&d), R, 18.);
    assert_eq!(left.len(), d.blocks().len());
    assert_eq!(left[0].diff_type, DiffType::Inserted);
    assert!(left[0].start_y == left[0].end_y);
    assert!(right[0].start_y <= right[0].end_y);
    let content = Folding::none(&d).layout(R, 18.).content_height();
    assert!(right[0].end_y < content);
}

fn settings() -> Vec<FoldSettings> {
    CONTEXT_RANGE_MODES
        .iter()
        .map(|&context_range| FoldSettings { context_range })
        .collect()
}

#[test]
fn folds_ligger_i_gapene_pa_alle_nivaer() {
    for (name, d) in all() {
        for s in settings() {
            let f = Folding::new(&d, s);
            let Some(range) = s.context_range else {
                assert!(f.groups().is_empty(), "{name}");
                continue;
            };
            for (i, side) in [L, R].into_iter().enumerate() {
                let count = d.line_count(side);
                // Gapene per side: før første blokk, mellom og etter siste
                let mut bounds = vec![0];
                for b in d.blocks() {
                    bounds.extend([b.lines[i].start, b.lines[i].end]);
                }
                bounds.push(count);
                let gaps: Vec<(usize, usize)> = bounds.chunks(2).map(|c| (c[0], c[1])).collect();
                for group in f.groups() {
                    let mut outer: Option<std::ops::Range<usize>> = None;
                    for (level, block) in group.iter().enumerate() {
                        let Some(r) = block.lines(side) else { continue };
                        assert!(r.len() >= 2 && r.end <= count, "{name} {r:?}");
                        let shift = range << level;
                        let g = gaps
                            .iter()
                            .position(|(s, e)| *s <= r.start && r.end <= *e)
                            .unwrap_or_else(|| panic!("{name} {side:?} {r:?} utenfor gap"));
                        let (gs, ge) = gaps[g];
                        // Første og siste gap går helt ut til kanten
                        let start = if g == 0 { 0 } else { gs + shift };
                        let end = if g == gaps.len() - 1 {
                            count
                        } else {
                            ge - shift
                        };
                        assert_eq!(r, start..end, "{name} {side:?} nivå {level}");
                        if let Some(o) = &outer {
                            assert!(o.start <= r.start && r.end <= o.end, "{name}");
                        }
                        outer = Some(r);
                    }
                }
            }
        }
    }
}

#[test]
fn kotlin_folds_per_niva() {
    let d = load("kotlin-realistic-edit");
    let blocks: Vec<_> = d.blocks().iter().map(|b| b.lines.clone()).collect();
    let visible = |range| {
        let f = Folding::new(
            &d,
            FoldSettings {
                context_range: Some(range),
            },
        );
        let v: Vec<_> = f.visible(L).map(|(_, v)| v.lines.clone()).collect();
        (v, f.layout(L, 18.).visual_line_count())
    };
    assert_eq!(blocks[5..7], [[45..46, 42..43], [47..47, 44..45]]);
    assert_eq!(
        visible(1),
        (vec![0..2, 4..9, 11..18, 33..37, 40..44, 48..56], 35)
    );
    assert_eq!(visible(2), (vec![5..8, 12..17, 34..36, 41..43, 49..55], 46));
    // Standardnivået: bare gapet 47..57 er langt nok, 4 linjer kontekst på hver side
    let (folds, count) = visible(4);
    assert_eq!((folds.len(), count), (1, 58));
    assert_eq!(folds[0], 51..53);
    assert_eq!(visible(8), (vec![], 59));
}

#[test]
fn scroll_og_navigasjon_med_folds() {
    for (name, d) in all() {
        for s in settings() {
            let f = Folding::new(&d, s);
            let layouts = f.layouts(VP.line_height);
            let mut state = ScrollState::default();
            let mut y = 0.;
            while y <= layouts[0].max_scroll(VP.height) + 50. {
                state.scroll_to(&d, &f, L, y, VP);
                assert!(state.y[1] >= 0. && state.y[1] <= layouts[1].max_scroll(VP.height));
                for sep in divider_separators(&f, state.y, VP, 24.) {
                    assert!(!sep.path.is_empty(), "{name}");
                }
                y += 13.;
            }
            if d.blocks().is_empty() {
                continue;
            }
            let mut block = 0;
            let mut t = target_for_block(&d, &f, block, VP);
            loop {
                for (i, side) in [L, R].into_iter().enumerate() {
                    let layout = &layouts[i];
                    let line = d.blocks()[block].lines(side).start;
                    // Endringer ligger aldri i en fold
                    assert_eq!(f.caret_after(side, line), line, "{name}");
                    assert!(t.scroll_y[i] <= layout.max_scroll(VP.height), "{name}");
                    let top = layout.line_top(line) - t.scroll_y[i];
                    assert!((0. ..VP.height).contains(&top), "{name} {s:?} {t:?}");
                }
                let Some(next) = next_block(&d, R, t.caret_lines[1]) else {
                    break;
                };
                block = next;
                t = target_for_block(&d, &f, block, VP);
            }
            assert_eq!(block, d.blocks().len() - 1, "{name}");
        }
    }
}

/// Samme rekkefølge som UI-et: lagre posisjon, endre folds, flytt caret og gjenopprett.
#[test]
fn fold_operasjoner_holder_seg_innenfor() {
    for (name, d) in all() {
        let mut f = Folding::new(&d, FoldSettings::default());
        let mut state = ScrollState::default();
        let mut carets = [0, 0];
        let lines = d.line_count(L).max(d.line_count(R));
        for step in 0..40usize {
            let side = if step % 2 == 0 { L } else { R };
            let line = (step * 7) % lines.min(d.line_count(side));
            carets[step % 2] = line;
            let keeper = PositionKeeper::save(&state, &f, carets, VP);
            match step % 5 {
                0 => {
                    f.expand_at(side, line);
                }
                1 => {
                    f.expand_all(false);
                }
                2 => f = f.with_settings(&d, f.settings().next_context_range()),
                3 => {
                    f.expand_all(true);
                }
                _ => {
                    if let Some(id) = f.fold_at_line(side, line) {
                        f.set_expanded(id, true);
                    }
                }
            }
            carets = [L, R].map(|s| f.caret_after(s, carets[if s == L { 0 } else { 1 }]));
            keeper.restore(&mut state, &d, &f, carets, VP);
            for (i, s) in [L, R].into_iter().enumerate() {
                let layout = f.layout(s, VP.line_height);
                assert!(state.y[i] >= 0. && state.y[i] <= layout.max_scroll(VP.height));
                // Caret er aldri skjult i en fold
                assert_eq!(f.caret_after(s, carets[i]), carets[i], "{name}");
                assert_eq!(
                    stripe_markers(&d, &f, s, VP.line_height).len(),
                    d.blocks().len(),
                    "{name}"
                );
            }
            divider_separators(&f, state.y, VP, 24.);
            state.scroll_by(&d, &f, side, 97., VP);
        }
    }
}

/// Cursoren står på en gyldig linje, aldri skjult i en fold, og scroll er innenfor.
fn assert_cursor_valid(name: &str, s: &DiffState, vp: Viewport, cmd: Command) {
    let c = s.cursor;
    let d = &s.diff;
    assert!(
        c.line < d.line_count(c.side),
        "{name} {cmd:?}: linje {}",
        c.line
    );
    assert_eq!(
        s.folding.caret_after(c.side, c.line),
        c.line,
        "{name} {cmd:?}: i fold"
    );
    let t = s.transferred_cursor();
    assert!(t.line < d.line_count(t.side), "{name} {cmd:?}: overført");
    for side in [L, R] {
        let layout = s.folding.layout(side, vp.line_height);
        let y = s.scroll.y(side);
        assert!(
            y >= 0. && y <= layout.max_scroll(vp.height) + 0.01,
            "{name} {cmd:?}"
        );
    }
    assert!(s.scroll.x >= 0. && s.scroll.x <= 2000., "{name} {cmd:?}: x");
}

#[test]
fn cursor_holder_seg_gyldig() {
    use Command::*;
    let cmds = [
        MoveDown,
        MoveUp,
        ScrollLeft,
        ScrollRight,
        ScrollToStart,
        ScrollToEnd,
        GoToTop,
        GoToBottom,
        HalfPageDown,
        HalfPageUp,
        NextChange,
        PrevChange,
        FocusSide(L),
        FocusSide(R),
        FoldOpen,
        ToggleFolds,
        NextContextRange,
        CopyLocation,
    ];
    let small = Viewport {
        height: 60.,
        line_height: 18.,
    };
    let hx = Horizontal {
        text_width: 300.,
        char_width: 8.,
        max_x: 2000.,
    };
    for (name, diff) in all() {
        let diff = Rc::new(diff);
        for vp in [VP, small] {
            let folding = Rc::new(Folding::new(&diff, FoldSettings::default()));
            let mut s = DiffState::new(diff.clone(), folding);
            s.init(vp);
            assert_cursor_valid(&name, &s, vp, MoveDown);
            // Deterministisk pseudotilfeldig rekkefølge
            let mut seed: u32 = 7;
            for _ in 0..400 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let cmd = cmds[(seed >> 16) as usize % cmds.len()];
                s.execute(cmd, vp, hx);
                assert_cursor_valid(&name, &s, vp, cmd);
                if seed.is_multiple_of(5) {
                    s.wheel(if seed.is_multiple_of(2) { L } else { R }, 37.5, vp);
                    assert_cursor_valid(&name, &s, vp, cmd);
                }
            }
        }
    }
}
