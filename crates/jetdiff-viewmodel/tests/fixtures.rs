//! The view model against fixtures from `fixtures/`, with known numbers for unicode and missing
//! line breaks, and invariants over all of them.

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use jetdiff_viewmodel::nav::{next_block, prev_block, target_for_block};
use jetdiff_viewmodel::{
    Command, ComparisonPolicy, DiffState, DiffType, FileDiff, Folding, Horizontal, InlineHighlight,
    PositionKeeper, ScrollState, Side, Viewport, divider_bands, divider_separators, stripe_markers,
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
fn norwegian_text_gives_byte_columns() {
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
fn emoji_with_empty_markers() {
    let d = load("unicode-emoji");
    assert_eq!(
        d.line_highlight(L, 0).inline,
        [h(8..12, DiffType::Modified)]
    );
    assert_eq!(&d.line(L, 0)[8..12], "🚀");
    // Empty sides of word fragments become 2px strokes
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
fn missing_line_break_on_left() {
    let d = load("missing-trailing-newline-left");
    assert_eq!(d.line_count(L), 3);
    assert_eq!(d.line_count(R), 4);
    let b = &d.blocks()[0];
    assert_eq!(b.lines, [3..3, 3..4]);
    assert_eq!(b.diff_type, DiffType::Inserted);
    // `Some(vec![])`: ignored color, no words
    assert_eq!(b.inner, Some(vec![]));
    let right = d.line_highlight(R, 3);
    assert!(d.blocks()[right.block.unwrap()].ignored());
    assert!(right.inline.is_empty());
    // The hunk is after the last Left line, so the 2px band goes below line 2
    let none = Folding::none(&d);
    let layout = none.layout(L, 18.);
    assert_eq!(layout.block_band(b.lines(L)), (53., 55.));
    let band = divider_bands(&d, &Folding::none(&d), [0., 0.], VP, 24.)[0];
    assert_eq!((band.top.p0.y, band.bottom.p0.y), (53., 55.));
    assert_eq!((band.top.p3.y, band.bottom.p3.y), (54., 72.));
}

#[test]
fn missing_line_break_on_right() {
    let d = load("missing-trailing-newline-right");
    let b = &d.blocks()[0];
    assert_eq!(b.lines, [3..4, 3..3]);
    assert_eq!(b.diff_type, DiffType::Deleted);
}

#[test]
fn missing_line_break_and_change() {
    let d = load("missing-trailing-newline-both-changed");
    assert_eq!(d.blocks()[0].lines, [2..3, 2..3]);
    let left = d.line_highlight(L, 2);
    assert_eq!(left.inline, [h(9..9, DiffType::Inserted)]);
    let right = d.line_highlight(R, 2);
    // Last line without a line break, so the color stops at the text
    assert_eq!(right.inline, [h(9..d.line(R, 2).len(), DiffType::Inserted)]);
    assert_eq!(d.line(R, 2).len(), 17);
}

#[test]
fn highlights_lie_on_char_boundaries() {
    for (name, d) in all() {
        for side in [L, R] {
            let blocks_lines: Vec<_> = d.blocks().iter().map(|b| b.lines(side)).collect();
            for line in 0..d.line_count(side) {
                let text = d.line(side, line);
                let hl = d.line_highlight(side, line);
                let in_block = blocks_lines.iter().any(|r| r.contains(&line));
                assert_eq!(hl.block.is_some(), in_block, "{name} {side:?} {line}");
                if hl.block.is_some_and(|b| !d.blocks()[b].ignored()) {
                    assert!(hl.inline.is_empty(), "{name}: whole line has no words");
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
fn scroll_and_divider_stay_within_bounds() {
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
                // Closed interval: a hunk that ends on the first painted line is included
                let min = -VP.line_height - 1.;
                assert!(band.bottom.p0.y >= min || band.bottom.p3.y >= min, "{name}");
            }
            y += 7.;
        }
    }
}

#[test]
fn navigation_visits_all_hunks() {
    for (name, d) in all() {
        if d.blocks().is_empty() {
            continue;
        }
        let f = Folding::none(&d);
        // The start position is the first hunk, as in `InitialScrollPositionSupport`
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
        // And back again
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

#[test]
fn one_fold_per_run_with_four_lines_of_context() {
    const CONTEXT: usize = 4;
    for (name, d) in all() {
        let f = Folding::new(&d);
        for (i, side) in [L, R].into_iter().enumerate() {
            let count = d.line_count(side);
            // The runs of unchanged lines per side: before the first hunk, between, and after the last
            let mut bounds = vec![0];
            for b in d.blocks() {
                bounds.extend([b.lines[i].start, b.lines[i].end]);
            }
            bounds.push(count);
            let runs: Vec<(usize, usize)> = bounds.chunks(2).map(|c| (c[0], c[1])).collect();
            let last = runs.len() - 1;
            // The first and last run fold all the way out to the edge
            let expected: Vec<_> = runs
                .iter()
                .enumerate()
                .map(|(run, &(start, end))| {
                    let start = if run == 0 { 0 } else { start + CONTEXT };
                    let end = if run == last {
                        count
                    } else {
                        end.saturating_sub(CONTEXT)
                    };
                    start..end
                })
                .filter(|lines| lines.end >= lines.start + 2)
                .collect();
            let folds: Vec<_> = f
                .folds()
                .iter()
                .filter_map(|fold| fold.lines(side))
                .collect();
            assert_eq!(folds, expected, "{name} {side:?}");
        }
    }
}

#[test]
fn opening_a_fold_shows_the_whole_run() {
    for (name, d) in all() {
        let mut f = Folding::new(&d);
        let folds = f.folds().to_vec();
        for (id, fold) in folds.iter().enumerate() {
            let side = if fold.lines(L).is_some() { L } else { R };
            let lines = fold.lines(side).unwrap();
            assert_eq!(f.fold_at_line(side, lines.start), Some(id), "{name}");
            assert!(f.set_expanded(id, true), "{name}");
            assert!(
                lines.clone().all(|line| f.caret_after(side, line) == line),
                "{name} {lines:?}: still folded"
            );
        }
        assert_eq!(f.visible(L).count() + f.visible(R).count(), 0, "{name}");
    }
}

#[test]
fn kotlin_folds() {
    let d = load("kotlin-realistic-edit");
    let blocks: Vec<_> = d.blocks().iter().map(|b| b.lines.clone()).collect();
    let f = Folding::new(&d);
    let folds: Vec<_> = f.visible(L).map(|(_, v)| v.lines.clone()).collect();
    assert_eq!(blocks[5..7], [[45..46, 42..43], [47..47, 44..45]]);
    // Only the gap 47..57 is long enough, 4 lines of context on each side
    assert_eq!(folds, vec![51..53]);
    assert_eq!(f.layout(L, 18.).visual_line_count(), 58);
}

#[test]
fn scroll_and_navigation_with_folds() {
    for (name, d) in all() {
        for f in [Folding::new(&d), Folding::none(&d)] {
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
                    // Hunks never lie in a fold
                    assert_eq!(f.caret_after(side, line), line, "{name}");
                    assert!(t.scroll_y[i] <= layout.max_scroll(VP.height), "{name}");
                    let top = layout.line_top(line) - t.scroll_y[i];
                    assert!((0. ..VP.height).contains(&top), "{name} {t:?}");
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

/// The same order as the UI: save position, change folds, move the caret and restore.
#[test]
fn fold_operations_stay_within_bounds() {
    for (name, d) in all() {
        let mut f = Folding::new(&d);
        let mut state = ScrollState::default();
        let mut carets = [0, 0];
        let lines = d.line_count(L).max(d.line_count(R));
        for step in 0..40usize {
            let side = if step % 2 == 0 { L } else { R };
            let line = (step * 7) % lines.min(d.line_count(side));
            carets[step % 2] = line;
            let keeper = PositionKeeper::save(&state, &f, carets, VP);
            match step % 4 {
                0 => {
                    f.expand_at(side, line);
                }
                1 => {
                    f.expand_all(false);
                }
                2 => {
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
                // The caret is never hidden in a fold
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

/// The cursor is on a valid line, never hidden in a fold, and scroll is within bounds.
fn assert_cursor_valid(name: &str, s: &DiffState, vp: Viewport, cmd: Command) {
    let c = s.cursor;
    let d = &s.diff;
    assert!(
        c.line < d.line_count(c.side),
        "{name} {cmd:?}: line {}",
        c.line
    );
    assert_eq!(
        s.folding.caret_after(c.side, c.line),
        c.line,
        "{name} {cmd:?}: in fold"
    );
    let t = s.transferred_cursor();
    assert!(t.line < d.line_count(t.side), "{name} {cmd:?}: transferred");
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
fn cursor_stays_valid() {
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
            let folding = Rc::new(Folding::new(&diff));
            let mut s = DiffState::new(diff.clone(), folding);
            s.init(vp);
            assert_cursor_valid(&name, &s, vp, MoveDown);
            // Deterministic pseudo-random order
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
