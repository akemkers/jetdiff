use super::*;
use jetdiff_core::ComparisonPolicy;

const LINE_HEIGHT: f32 = 10.;
/// 20 whole lines.
const VIEWPORT: Viewport = Viewport {
    height: 200.,
    line_height: LINE_HEIGHT,
};
const NO_HORIZONTAL: Horizontal = Horizontal {
    text_width: 0.,
    char_width: 0.,
    max_x: 0.,
};

fn file_diff(old: &str, new: &str) -> Rc<FileDiff> {
    Rc::new(FileDiff::new(old, new, ComparisonPolicy::Default).unwrap())
}

fn unified(old: &str, new: &str) -> UnifiedDiff {
    UnifiedDiff::new(file_diff(old, new))
}

fn lines(diff: &UnifiedDiff) -> Vec<[Option<usize>; 2]> {
    diff.rows().iter().map(|row| row.lines).collect()
}

fn numbered(prefix: &str, count: usize) -> String {
    (0..count).map(|i| format!("{prefix}{i}\n")).collect()
}

/// Hunks on lines 10, 40 and 90 of 100, one line each.
fn three_hunks() -> Rc<FileDiff> {
    let old = numbered("a", 100);
    let mut new: Vec<String> = old.lines().map(str::to_owned).collect();
    for i in [10, 40, 90] {
        new[i] = format!("x{i}");
    }
    file_diff(&old, &(new.join("\n") + "\n"))
}

fn state(diff: Rc<FileDiff>) -> UnifiedState {
    let mut state = UnifiedState::new(Rc::new(UnifiedDiff::new(diff)));
    state.init(VIEWPORT);
    state
}

fn run(state: &mut UnifiedState, command: Command) -> Outcome {
    state.execute(command, VIEWPORT, NO_HORIZONTAL)
}

#[test]
fn deleted_above_added() {
    let diff = unified("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(
        lines(&diff),
        vec![
            [Some(0), Some(0)],
            [Some(1), None],
            [None, Some(1)],
            [Some(2), Some(2)],
            [Some(3), Some(3)],
        ]
    );
    assert_eq!(diff.hunk_rows().len(), 1);
    assert_eq!(diff.hunk_rows()[0], 1..3);
    assert_eq!(diff.text(1), "b");
    assert_eq!(diff.text(2), "B");
    assert_eq!(diff.row(1).kind(), HunkKind::Deleted);
    assert_eq!(diff.row(2).kind(), HunkKind::Added);
}

#[test]
fn line_to_row() {
    let diff = unified("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(diff.row_of(Side::Left, 1), 1);
    assert_eq!(diff.row_of(Side::Right, 1), 2);
    // An unchanged line on Left has the same row as on Right
    assert_eq!(diff.row_of(Side::Left, 2), 3);
    assert_eq!(diff.row_of(Side::Right, 2), 3);
}

#[test]
fn added_file_has_only_new_numbers() {
    let diff = unified("", "a\nb\n");
    assert!(diff.rows().iter().all(|row| row.lines[0].is_none()));
    assert_eq!(diff.row_count(), 3);
    let diff = unified("a\nb\n", "");
    assert!(diff.rows().iter().all(|row| row.lines[1].is_none()));
    assert_eq!(diff.text(0), "a");
}

#[test]
fn modified_hunk_gives_two_marks() {
    let diff = unified("a\nb\nc\n", "a\nB\nc\n");
    let markers = diff.stripe_markers(LINE_HEIGHT);
    let types: Vec<_> = markers.iter().map(|marker| marker.kind).collect();
    assert_eq!(types, [HunkKind::Deleted, HunkKind::Added]);
    assert_eq!(markers[0].start_y, 10.);
    assert_eq!(markers[1].start_y, 20.);
}

#[test]
fn opens_on_first_hunk() {
    let state = state(three_hunks());
    // Line 10 deleted and added: row 10 and 11
    assert_eq!(state.row, 10);
    assert_eq!(state.cursor_hunk(), Some(0));
}

#[test]
fn n_and_p_land_on_start_of_hunk() {
    let mut state = state(three_hunks());
    assert!(run(&mut state, Command::NextHunk).animate);
    // Row 40 for the deleted one, plus the row for added line 10
    assert_eq!(state.row, 41);
    run(&mut state, Command::NextHunk);
    assert_eq!(state.row, 92);
    assert!(!run(&mut state, Command::NextHunk).changed);
    run(&mut state, Command::PreviousHunk);
    assert_eq!(state.row, 41);
    // Inside the hunk, p goes to the previous hunk, not the start of this one
    run(&mut state, Command::MoveDown);
    run(&mut state, Command::PreviousHunk);
    assert_eq!(state.row, 10);
}

#[test]
fn j_k_and_g_keep_scrolloff() {
    let mut state = state(file_diff(&numbered("a", 200), &numbered("a", 200)));
    for _ in 0..30 {
        run(&mut state, Command::MoveDown);
    }
    assert_eq!(state.row, 30);
    // 8 rows below the cursor in a window 20 rows high
    assert_eq!(state.scroll_y(), (30. + 1. + 8.) * LINE_HEIGHT - 200.);
    run(&mut state, Command::GoToBottom);
    assert_eq!(state.row, 200);
    assert!(!run(&mut state, Command::MoveDown).changed);
    run(&mut state, Command::GoToTop);
    assert_eq!((state.row, state.scroll_y()), (0, 0.));
}

#[test]
fn deleted_row_gives_line_below_on_right() {
    let mut state = state(three_hunks());
    assert_eq!(state.location(), Cursor::new(Side::Right, 10));
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 10));
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 11));
}

#[test]
fn switching_keeps_line_and_height() {
    let diff = three_hunks();
    let mut split = DiffState::new(diff.clone(), Rc::new(Folding::none(&diff)));
    split.init(VIEWPORT);
    for _ in 0..35 {
        split.execute(Command::MoveDown, VIEWPORT, NO_HORIZONTAL);
    }
    let top = |line: usize, scroll_y: f32| line as f32 * LINE_HEIGHT - scroll_y;
    let before = top(split.cursor.line, split.scroll.y(Side::Right));

    let unified = UnifiedState::from_split(&split, VIEWPORT);
    let row = unified.diff.row(unified.row);
    assert_eq!((row.side(), row.line()), (Side::Right, split.cursor.line));
    assert_eq!(top(unified.row, unified.scroll_y()), before);

    let back = unified.to_split(Rc::new(Folding::none(&diff)), VIEWPORT);
    assert_eq!(back.cursor, split.cursor);
    assert_eq!(top(back.cursor.line, back.scroll.y(Side::Right)), before);
}

#[test]
fn deleted_row_gives_left_side_in_split() {
    let diff = three_hunks();
    let state = state(diff.clone());
    let split = state.to_split(Rc::new(Folding::none(&diff)), VIEWPORT);
    assert_eq!(split.cursor, Cursor::new(Side::Left, 10));
    let back = UnifiedState::from_split(&split, VIEWPORT);
    assert_eq!(back.row, 10);
}

#[test]
fn wheel_pushes_the_cursor() {
    let mut state = state(file_diff(&numbered("a", 200), &numbered("a", 200)));
    assert!(state.wheel(100., VIEWPORT));
    // Scrolloff 8 below the top
    assert_eq!(state.row, 18);
    // A little back: the cursor still has scrolloff
    assert!(!state.wheel(-5., VIEWPORT));
    assert_eq!(state.row, 18);
}

#[test]
fn new_text_keeps_the_line() {
    let mut state = state(three_hunks());
    run(&mut state, Command::NextHunk);
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 40));
    // The hunk on line 10 is gone, so the rows move
    let old = numbered("a", 100);
    let new = old.replace("a40\n", "x40\n");
    state.replace_diff(file_diff(&old, &new), VIEWPORT);
    assert_eq!(state.location(), Cursor::new(Side::Right, 40));
}

#[test]
fn split_places_lines_in_pairs() {
    let diff =
        UnifiedDiff::with_layout(file_diff("a\nb\nc\nd\n", "a\nB\nd\ne\n"), RowLayout::Split);
    let rows = lines(&diff);
    // `b c` against `B`, and `e` is added
    assert_eq!(rows[0], [Some(0), Some(0)]);
    assert_eq!(rows[1], [Some(1), Some(1)]);
    assert_eq!(rows[2], [Some(2), None]);
    assert_eq!(rows[3], [Some(3), Some(2)]);
    assert_eq!(rows[4], [None, Some(3)]);
    assert_eq!(diff.hunk_rows(), [1..3, 4..5]);
    assert_eq!(diff.row_of(Side::Right, 2), 3);
}

#[test]
fn realign_keeps_the_line() {
    let mut state = UnifiedState::new(Rc::new(unified("a\nb\nc\n", "a\nB\nc\n")));
    state.init(VIEWPORT);
    // Unified: a, b, B, c. The cursor on c.
    state.click(3);
    let split = state.realign(RowLayout::Split, VIEWPORT);
    assert_eq!(split.diff.row_layout(), RowLayout::Split);
    assert_eq!(split.diff.row(split.row).lines, [Some(2), Some(2)]);
    let back = split.realign(RowLayout::Unified, VIEWPORT);
    assert_eq!(back.row, 3);
}

#[test]
fn split_pairs_lines_with_common_text() {
    let old = "a\nuse x::{\n    A,\n};\nuse app::{Keys, B};\nz\n";
    let new = "a\nuse app::C;\nz\n";
    let diff = UnifiedDiff::with_layout(file_diff(old, new), RowLayout::Split);
    let rows = lines(&diff);
    // `use x::{ A, };` is gone, `use app::` sits against `use app::`
    assert_eq!(rows[1], [Some(1), None]);
    assert_eq!(rows[3], [Some(3), None]);
    assert_eq!(rows[4], [Some(4), Some(1)]);
    assert_eq!(rows[5], [Some(5), Some(2)]);
}
