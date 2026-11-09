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

/// Hunks on lines 10, 40 and 90 of 100.
fn three_hunks() -> FileDiff {
    let a = lines("a", 100);
    let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
    for i in [10, 40, 90] {
        b[i] = format!("x{i}");
    }
    diff(&a, &(b.join("\n") + "\n"))
}

fn both(r: Range<usize>) -> [Option<Range<usize>>; 2] {
    [Some(r.clone()), Some(r)]
}

fn visible(folding: &Folding, side: Side) -> Vec<Range<usize>> {
    folding
        .visible(side)
        .map(|(_, fold)| fold.lines.clone())
        .collect()
}

#[test]
fn one_fold_per_run_with_four_lines_of_context() {
    let folding = Folding::new(&three_hunks());
    let folds: Vec<_> = folding
        .folds()
        .iter()
        .map(|fold| fold.lines.clone())
        .collect();
    assert_eq!(
        folds,
        // 101 lines with the empty one after the last '\n'. The first and last fold go all the
        // way to the edge.
        [both(0..6), both(15..36), both(45..86), both(95..101)]
    );
    assert_eq!(visible(&folding, L), [0..6, 15..36, 45..86, 95..101]);
}

#[test]
fn at_least_two_lines_per_side() {
    // Run of 9 lines: 4 context on each side leaves one line, so no fold
    let folds = build_folds([[0..1, 0..1], [10..11, 10..11]], [11, 11]);
    assert!(folds.is_empty());
    // Different length: only the long side gets a fold
    let folds = build_folds([[0..1, 0..1], [10..11, 20..21]], [11, 21]);
    assert_eq!(folds, [[None, Some(5..16)]]);
}

#[test]
fn equal_files_fold_completely() {
    let diff = diff(&lines("a", 30), &lines("a", 30));
    let folding = Folding::new(&diff);
    assert_eq!(folding.folds().len(), 1);
    assert_eq!(folding.folds()[0].lines, both(0..31));
    assert_eq!(folding.layout(L, 10.).visual_line_count(), 1);
}

#[test]
fn context_off_gives_no_folds() {
    let diff = three_hunks();
    let folding = Folding::none(&diff);
    assert!(folding.folds().is_empty());
    assert_eq!(folding.layout(L, 10.).visual_line_count(), 101);
    assert_eq!(folding.closed_folds().count(), 0);
}

#[test]
fn opening_a_fold_shows_the_whole_run() {
    let diff = three_hunks();
    let mut folding = Folding::new(&diff);
    let id = folding.fold_at_line(L, 45).unwrap();
    assert_eq!(id, 2);
    assert_eq!(folding.fold_at_line(R, 45), Some(id));
    assert_eq!(folding.fold_at_line(L, 46), None);

    assert!(folding.set_open(id, true));
    // No smaller fold is left inside 45..86, and both sides follow
    assert_eq!(visible(&folding, L), [0..6, 15..36, 95..101]);
    assert_eq!(visible(&folding, R), visible(&folding, L));
    assert!((45..86).all(|line| folding.fold_at_line(L, line).is_none()));
    // The folds 0..6, 15..36 and 95..101 each still take one visual line
    assert_eq!(folding.layout(L, 10.).visual_line_count(), 101 - 5 - 20 - 5);
}

#[test]
fn open_and_close_all() {
    let diff = three_hunks();
    let mut folding = Folding::new(&diff);
    assert!(folding.set_all_open(true));
    assert_eq!(folding.visible(L).count(), 0);
    assert_eq!(folding.layout(R, 10.).visual_line_count(), 101);
    assert!(!folding.set_all_open(true));
    folding.set_all_open(false);
    assert_eq!(folding.visible(L).count(), 4);
}

#[test]
fn open_at_opens_the_fold_containing_the_line() {
    let diff = three_hunks();
    let mut folding = Folding::new(&diff);
    // Inside the fold 45..86, not on its first line
    assert!(folding.open_at(L, 60));
    assert_eq!(visible(&folding, L), [0..6, 15..36, 95..101]);
    assert!(!folding.open_at(L, 60));
    // On the first line
    assert!(folding.open_at(R, 0));
    assert_eq!(visible(&folding, L), [15..36, 95..101]);
    // Inside a hunk there is nothing
    assert!(!folding.open_at(L, 10));
}

#[test]
fn cursor_in_closed_fold_moves_to_start() {
    let diff = three_hunks();
    let folding = Folding::new(&diff);
    assert_eq!(folding.cursor_after(L, 50), 45);
    assert_eq!(folding.cursor_after(L, 45), 45);
    assert_eq!(folding.cursor_after(L, 40), 40);
    assert_eq!(folding.cursor_after(L, 3), 0);
}

#[test]
fn fold_links_need_fold_on_both_sides() {
    // Right has 20 extra lines after line 10, so the last fold starts at 14 and 34
    let a = lines("a", 30);
    let b = format!(
        "{}{}{}",
        lines("a", 10),
        lines("x", 20),
        &a[lines("a", 10).len()..]
    );
    let diff = diff(&a, &b);
    assert_eq!(diff.hunks()[0].lines, [10..10, 10..30]);
    let mut folding = Folding::new(&diff);
    let closed: Vec<_> = folding.closed_folds().collect();
    assert_eq!(closed.len(), 2);
    assert_eq!(closed[0].lines, [0, 0]);
    assert_eq!(closed[1].lines, [14, 34]);
    folding.set_hovered(Some(closed[1].id));
    assert!(folding.closed_folds().nth(1).unwrap().hovered);

    // Only one side has a fold: no fold link
    let one_side = Fold {
        lines: [None, Some(5..16)],
        open: false,
    };
    let folding = Folding::with_folds([11, 21], vec![one_side]);
    assert_eq!(folding.fold_at_line(R, 5), Some(0));
    assert_eq!(folding.closed_folds().count(), 0);
}

#[test]
fn hover_resets_when_fold_disappears() {
    let diff = three_hunks();
    let mut folding = Folding::new(&diff);
    let id = folding.fold_at_line(L, 45).unwrap();
    assert!(folding.set_hovered(Some(id)));
    assert!(!folding.set_hovered(Some(id)));
    folding.set_open(id, true);
    assert_eq!(folding.hovered(), None);
}

#[test]
fn unified_folds_the_same_unchanged_lines_as_split() {
    let diff = std::rc::Rc::new(three_hunks());
    let unified = UnifiedDiff::new(diff.clone());
    let folding = UnifiedFolding::unified(&unified);
    let rows: Vec<_> = folding
        .folds()
        .iter()
        .map(|fold| fold.lines[0].clone().unwrap())
        .collect();
    // Each hunk is a deleted and an added row. 4 rows of context, one fold per run.
    assert_eq!(rows, [0..6, 16..37, 47..88, 98..104]);
    let right_side_lines: Vec<_> = rows
        .iter()
        .map(|rows| {
            let line = |row: usize| unified.row(row).lines[idx(R)].unwrap();
            line(rows.start)..line(rows.end - 1) + 1
        })
        .collect();
    let split: Vec<_> = Folding::new(&diff)
        .folds()
        .iter()
        .map(|fold| fold.lines(R).unwrap())
        .collect();
    assert_eq!(right_side_lines, split);
    assert_eq!(
        folding.layout(10.).visual_line_count(),
        104 - 5 - 20 - 40 - 5
    );
}

#[test]
fn unified_cursor_in_closed_fold_moves_to_start() {
    let unified = UnifiedDiff::new(std::rc::Rc::new(three_hunks()));
    let mut folding = UnifiedFolding::unified(&unified);
    assert_eq!(folding.cursor_after(60), 47);
    assert_eq!(folding.fold_at_row(47), Some(2));
    assert!(folding.open_at(60));
    assert_eq!(folding.cursor_after(60), 60);
    assert_eq!(folding.visible().count(), 3);
}
