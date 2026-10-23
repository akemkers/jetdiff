//! The view model against fixtures from `fixtures/`, with known numbers for unicode and missing
//! line breaks, and invariants over all of them.

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use jetdiff_viewmodel::nav::{next_hunk, previous_hunk, target_for_hunk};
use jetdiff_viewmodel::{
    Command, ComparisonPolicy, DiffState, FileDiff, Folding, Horizontal, HunkKind, InlineHighlight,
    PositionKeeper, ScrollState, Side, Viewport, divider_bands, fold_links, hunk_minimap_marks,
};

const L: Side = Side::Left;
const R: Side = Side::Right;

const VIEWPORT: Viewport = Viewport {
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

fn h(cols: std::ops::Range<usize>, kind: HunkKind) -> InlineHighlight {
    InlineHighlight {
        cols,
        kind,
        past_line_end: false,
    }
}

#[test]
fn norwegian_text_gives_byte_columns() {
    let diff = load("unicode-norwegian");
    let left_side_line_0 = diff.line_highlight(L, 0);
    let hunk = &diff.hunks()[left_side_line_0.hunk.unwrap()];
    assert_eq!(hunk.kind, HunkKind::Modified);
    assert!(hunk.compared_by_word());
    assert_eq!(left_side_line_0.inline, [h(22..32, HunkKind::Modified)]);
    assert_eq!(&diff.line(L, 0)[22..32], "brødskiva");
    let left_side_line_1 = diff.line_highlight(L, 1);
    assert_eq!(left_side_line_1.inline, [h(20..24, HunkKind::Modified)]);
    assert_eq!(&diff.line(L, 1)[20..24], "godt");
    assert_eq!(diff.line_highlight(R, 0).inline[0].cols, 22..33);
}

#[test]
fn emoji_with_empty_markers() {
    let diff = load("unicode-emoji");
    assert_eq!(
        diff.line_highlight(L, 0).inline,
        [h(8..12, HunkKind::Modified)]
    );
    assert_eq!(&diff.line(L, 0)[8..12], "🚀");
    // Empty sides of inline highlights become 2px strokes
    assert_eq!(
        diff.line_highlight(L, 1).inline,
        [h(8..8, HunkKind::Added), h(11..11, HunkKind::Added)]
    );
    assert_eq!(
        diff.line_highlight(R, 1).inline,
        [h(8..12, HunkKind::Added), h(15..20, HunkKind::Added)]
    );
    assert_eq!(&diff.line(R, 1)[15..20], " done");
    assert_eq!(diff.line_highlight(L, 2).hunk, None);
}

#[test]
fn missing_line_break_on_left_side() {
    let diff = load("missing-trailing-newline-left");
    assert_eq!(diff.line_count(L), 3);
    assert_eq!(diff.line_count(R), 4);
    let hunk = &diff.hunks()[0];
    assert_eq!(hunk.lines, [3..3, 3..4]);
    assert_eq!(hunk.kind, HunkKind::Added);
    // `Some(vec![])`: compared by word, no words differ
    assert_eq!(hunk.inline_highlights, Some(vec![]));
    let right_side = diff.line_highlight(R, 3);
    assert!(diff.hunks()[right_side.hunk.unwrap()].compared_by_word());
    assert!(right_side.inline.is_empty());
    // The hunk is after the last Left line, so the 2px band goes below line 2
    let none = Folding::none(&diff);
    let layout = none.layout(L, 18.);
    assert_eq!(layout.hunk_band(hunk.lines(L)), (53., 55.));
    let band = divider_bands(&diff, &Folding::none(&diff), [0., 0.], VIEWPORT, 24.)[0];
    assert_eq!((band.top.p0.y, band.bottom.p0.y), (53., 55.));
    assert_eq!((band.top.p3.y, band.bottom.p3.y), (54., 72.));
}

#[test]
fn missing_line_break_on_right_side() {
    let diff = load("missing-trailing-newline-right");
    let hunk = &diff.hunks()[0];
    assert_eq!(hunk.lines, [3..4, 3..3]);
    assert_eq!(hunk.kind, HunkKind::Deleted);
}

#[test]
fn missing_line_break_on_both_sides() {
    let diff = load("missing-trailing-newline-both-changed");
    assert_eq!(diff.hunks()[0].lines, [2..3, 2..3]);
    let left_side = diff.line_highlight(L, 2);
    assert_eq!(left_side.inline, [h(9..9, HunkKind::Added)]);
    let right_side = diff.line_highlight(R, 2);
    // Last line without a line break, so the color stops at the text
    assert_eq!(
        right_side.inline,
        [h(9..diff.line(R, 2).len(), HunkKind::Added)]
    );
    assert_eq!(diff.line(R, 2).len(), 17);
}

#[test]
fn highlights_lie_on_char_boundaries() {
    for (name, diff) in all() {
        for side in [L, R] {
            let hunk_lines: Vec<_> = diff.hunks().iter().map(|hunk| hunk.lines(side)).collect();
            for line in 0..diff.line_count(side) {
                let text = diff.line(side, line);
                let hl = diff.line_highlight(side, line);
                let in_hunk = hunk_lines.iter().any(|r| r.contains(&line));
                assert_eq!(hl.hunk.is_some(), in_hunk, "{name} {side:?} {line}");
                if hl
                    .hunk
                    .is_some_and(|hunk| !diff.hunks()[hunk].compared_by_word())
                {
                    assert!(hl.inline.is_empty(), "{name}: whole line has no words");
                }
                for i in hl.inline {
                    assert!(i.cols.end <= text.len(), "{name} {side:?} {line} {i:?}");
                    assert!(text.is_char_boundary(i.cols.start), "{name} {i:?}");
                    assert!(text.is_char_boundary(i.cols.end), "{name} {i:?}");
                    if i.past_line_end {
                        assert!(line + 1 < diff.line_count(side), "{name} {i:?}");
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
            Folding::none(&d).layout(L, 18.).max_scroll(VIEWPORT.height),
            Folding::none(&d).layout(R, 18.).max_scroll(VIEWPORT.height),
        ];
        let mut y = 0.;
        while y <= max[0] + 50. {
            s.scroll_to(&d, &Folding::none(&d), L, y, VIEWPORT);
            assert!(s.y[1] >= 0. && s.y[1] <= max[1], "{name} {s:?}");
            for band in divider_bands(&d, &Folding::none(&d), s.y, VIEWPORT, 24.) {
                assert!(band.top.p0.y < band.bottom.p0.y, "{name}");
                assert!(band.top.p3.y < band.bottom.p3.y, "{name}");
                // Closed interval: a hunk that ends on the first painted line is included
                let min = -VIEWPORT.line_height - 1.;
                assert!(band.bottom.p0.y >= min || band.bottom.p3.y >= min, "{name}");
            }
            y += 7.;
        }
    }
}

#[test]
fn navigation_visits_all_hunks() {
    for (name, diff) in all() {
        if diff.hunks().is_empty() {
            continue;
        }
        let folding = Folding::none(&diff);
        // The start position is the first hunk, as in `InitialScrollPositionSupport`
        let mut seen = vec![0];
        let mut cursor_line = target_for_hunk(&diff, &folding, 0, VIEWPORT).cursor_lines[1];
        while let Some(hunk) = next_hunk(&diff, R, cursor_line) {
            seen.push(hunk);
            let target = target_for_hunk(&diff, &folding, hunk, VIEWPORT);
            cursor_line = target.cursor_lines[1];
            for (i, side) in [L, R].into_iter().enumerate() {
                assert!(target.scroll_y[i] >= 0., "{name}");
                assert!(
                    target.scroll_y[i]
                        <= Folding::none(&diff)
                            .layout(side, 18.)
                            .max_scroll(VIEWPORT.height),
                    "{name}"
                );
            }
        }
        let expected: Vec<usize> = (0..diff.hunks().len()).collect();
        assert_eq!(seen, expected, "{name}");
        // And back again
        let mut back = vec![];
        while let Some(hunk) = previous_hunk(&diff, R, cursor_line) {
            back.push(hunk);
            cursor_line = target_for_hunk(&diff, &folding, hunk, VIEWPORT).cursor_lines[1];
        }
        back.reverse();
        assert_eq!(back, expected[..expected.len() - 1], "{name}");
    }
}

#[test]
fn hunk_minimap_per_side() {
    let diff = load("insert-middle");
    let left_side = hunk_minimap_marks(&diff, &Folding::none(&diff), L, 18.);
    let right_side = hunk_minimap_marks(&diff, &Folding::none(&diff), R, 18.);
    assert_eq!(left_side.len(), diff.hunks().len());
    assert_eq!(left_side[0].kind, HunkKind::Added);
    assert!(left_side[0].start_y == left_side[0].end_y);
    assert!(right_side[0].start_y <= right_side[0].end_y);
    let content = Folding::none(&diff).layout(R, 18.).content_height();
    assert!(right_side[0].end_y < content);
}

#[test]
fn one_fold_per_run_with_four_lines_of_context() {
    const CONTEXT: usize = 4;
    for (name, diff) in all() {
        let folding = Folding::new(&diff);
        for (i, side) in [L, R].into_iter().enumerate() {
            let count = diff.line_count(side);
            // The runs of unchanged lines per side: before the first hunk, between, and after the last
            let mut bounds = vec![0];
            for hunk in diff.hunks() {
                bounds.extend([hunk.lines[i].start, hunk.lines[i].end]);
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
            let folds: Vec<_> = folding
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
    for (name, diff) in all() {
        let mut folding = Folding::new(&diff);
        let folds = folding.folds().to_vec();
        for (id, fold) in folds.iter().enumerate() {
            let side = if fold.lines(L).is_some() { L } else { R };
            let lines = fold.lines(side).unwrap();
            assert_eq!(folding.fold_at_line(side, lines.start), Some(id), "{name}");
            assert!(folding.set_open(id, true), "{name}");
            assert!(
                lines
                    .clone()
                    .all(|line| folding.cursor_after(side, line) == line),
                "{name} {lines:?}: still folded"
            );
        }
        assert_eq!(
            folding.visible(L).count() + folding.visible(R).count(),
            0,
            "{name}"
        );
    }
}

#[test]
fn kotlin_folds() {
    let diff = load("kotlin-realistic-edit");
    let hunks: Vec<_> = diff.hunks().iter().map(|hunk| hunk.lines.clone()).collect();
    let folding = Folding::new(&diff);
    let folds: Vec<_> = folding.visible(L).map(|(_, v)| v.lines.clone()).collect();
    assert_eq!(hunks[5..7], [[45..46, 42..43], [47..47, 44..45]]);
    // Only the run 47..57 is long enough, 4 lines of context on each side
    assert_eq!(folds, vec![51..53]);
    assert_eq!(folding.layout(L, 18.).visual_line_count(), 58);
}

#[test]
fn scroll_and_navigation_with_folds() {
    for (name, diff) in all() {
        for folding in [Folding::new(&diff), Folding::none(&diff)] {
            let layouts = folding.layouts(VIEWPORT.line_height);
            let mut state = ScrollState::default();
            let mut y = 0.;
            while y <= layouts[0].max_scroll(VIEWPORT.height) + 50. {
                state.scroll_to(&diff, &folding, L, y, VIEWPORT);
                assert!(state.y[1] >= 0. && state.y[1] <= layouts[1].max_scroll(VIEWPORT.height));
                for link in fold_links(&folding, state.y, VIEWPORT, 24.) {
                    assert!(!link.path.is_empty(), "{name}");
                }
                y += 13.;
            }
            if diff.hunks().is_empty() {
                continue;
            }
            let mut hunk = 0;
            let mut target = target_for_hunk(&diff, &folding, hunk, VIEWPORT);
            loop {
                for (i, side) in [L, R].into_iter().enumerate() {
                    let layout = &layouts[i];
                    let line = diff.hunks()[hunk].lines(side).start;
                    // Hunks never lie in a fold
                    assert_eq!(folding.cursor_after(side, line), line, "{name}");
                    assert!(
                        target.scroll_y[i] <= layout.max_scroll(VIEWPORT.height),
                        "{name}"
                    );
                    let top = layout.line_top(line) - target.scroll_y[i];
                    assert!((0. ..VIEWPORT.height).contains(&top), "{name} {target:?}");
                }
                let Some(next) = next_hunk(&diff, R, target.cursor_lines[1]) else {
                    break;
                };
                hunk = next;
                target = target_for_hunk(&diff, &folding, hunk, VIEWPORT);
            }
            assert_eq!(hunk, diff.hunks().len() - 1, "{name}");
        }
    }
}

/// The same order as the UI: save position, change folds, move the cursor and restore.
#[test]
fn fold_operations_stay_within_bounds() {
    for (name, diff) in all() {
        let mut folding = Folding::new(&diff);
        let mut state = ScrollState::default();
        let mut cursor_lines = [0, 0];
        let lines = diff.line_count(L).max(diff.line_count(R));
        for step in 0..40usize {
            let side = if step % 2 == 0 { L } else { R };
            let line = (step * 7) % lines.min(diff.line_count(side));
            cursor_lines[step % 2] = line;
            let keeper = PositionKeeper::save(&state, &folding, cursor_lines, VIEWPORT);
            match step % 4 {
                0 => {
                    folding.open_at(side, line);
                }
                1 => {
                    folding.set_all_open(false);
                }
                2 => {
                    folding.set_all_open(true);
                }
                _ => {
                    if let Some(id) = folding.fold_at_line(side, line) {
                        folding.set_open(id, true);
                    }
                }
            }
            cursor_lines = [L, R].map(|side| {
                folding.cursor_after(side, cursor_lines[if side == L { 0 } else { 1 }])
            });
            keeper.restore(&mut state, &diff, &folding, cursor_lines, VIEWPORT);
            for (i, s) in [L, R].into_iter().enumerate() {
                let layout = folding.layout(s, VIEWPORT.line_height);
                assert!(state.y[i] >= 0. && state.y[i] <= layout.max_scroll(VIEWPORT.height));
                // The cursor is never hidden in a fold
                assert_eq!(
                    folding.cursor_after(s, cursor_lines[i]),
                    cursor_lines[i],
                    "{name}"
                );
                assert_eq!(
                    hunk_minimap_marks(&diff, &folding, s, VIEWPORT.line_height).len(),
                    diff.hunks().len(),
                    "{name}"
                );
            }
            fold_links(&folding, state.y, VIEWPORT, 24.);
            state.scroll_by(&diff, &folding, side, 97., VIEWPORT);
        }
    }
}

/// The cursor is on a valid line, never hidden in a fold, and scroll is within bounds.
fn assert_cursor_valid(name: &str, s: &DiffState, vp: Viewport, cmd: Command) {
    let c = s.cursor;
    let diff = &s.diff;
    assert!(
        c.line < diff.line_count(c.side),
        "{name} {cmd:?}: line {}",
        c.line
    );
    assert_eq!(
        s.folding.cursor_after(c.side, c.line),
        c.line,
        "{name} {cmd:?}: in fold"
    );
    let mirrored = s.mirrored_cursor();
    assert!(
        mirrored.line < diff.line_count(mirrored.side),
        "{name} {cmd:?}: mirrored"
    );
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
        NextHunk,
        PreviousHunk,
        FocusSide(L),
        FocusSide(R),
        FoldOpen,
        ToggleFolds,
        CopyCursorLocation,
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
        for vp in [VIEWPORT, small] {
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
