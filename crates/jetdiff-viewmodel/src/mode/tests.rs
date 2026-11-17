use super::*;
use jetdiff_core::ComparisonPolicy;
use jetdiff_core::side::Side;

const VIEWPORT: Viewport = Viewport {
    height: 200.,
    line_height: 10.,
};

fn diff(left_side: &str, right_side: &str) -> Rc<FileDiff> {
    Rc::new(FileDiff::new(left_side, right_side, ComparisonPolicy::Default).unwrap())
}

fn is_aligned_split(mode: &DiffMode) -> bool {
    matches!(mode.state(), ModeState::Rows(state) if state.diff.row_layout() == RowLayout::AlignedSplit)
}

#[test]
fn switch_back_and_forth() {
    let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let right_side = left_side.replace("a50\n", "x\n");
    let mut mode = DiffMode::new(
        diff(&left_side, &right_side),
        false,
        false,
        SplitRows::Synced,
    );
    mode.init(VIEWPORT);
    let before = mode.cursor_location();

    mode.toggle(false, VIEWPORT);
    assert!(mode.is_unified());
    assert_eq!(mode.cursor_location(), before);
    assert_eq!(mode.cursor_hunk(), Some(0));

    mode.toggle(false, VIEWPORT);
    assert!(!mode.is_unified());
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 50));
}

#[test]
fn switch_closes_the_folds_opened_with_o() {
    // One hunk on line 50 of 100, so the folds are 0..46 and 55..101
    let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let right_side = left_side.replace("a50\n", "x\n");
    let mut mode = DiffMode::new(
        diff(&left_side, &right_side),
        false,
        true,
        SplitRows::Synced,
    );
    mode.init(VIEWPORT);
    let mut run = |command| mode.execute(command, VIEWPORT, Horizontal::default());
    run(Command::GoToBottom);
    run(Command::FoldOpen);
    run(Command::GoToBottom);
    run(Command::MoveUp);
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 99));

    // The line is in a closed fold in Unified, so the cursor moves out to the fold
    mode.toggle(true, VIEWPORT);
    assert!(mode.is_unified());
    assert!(mode.any_fold_closed());
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 55));

    mode.toggle(true, VIEWPORT);
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 55));
}

#[test]
fn added_and_deleted_files_are_shown_unified() {
    let mode = DiffMode::new(diff("", "a\n"), false, true, SplitRows::Synced);
    assert!(mode.is_unified());

    let changed = diff("a\nb\n", "a\nc\n");
    let mut mode = DiffMode::new(changed.clone(), false, true, SplitRows::Synced);
    mode.init(VIEWPORT);
    mode.replace_diff(diff("a\nb\n", ""), false, true, VIEWPORT);
    assert!(mode.is_unified());
    mode.replace_diff(changed, false, true, VIEWPORT);
    assert!(!mode.is_unified());
}

#[test]
fn aligned_split_switches_to_unified_and_back() {
    let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let right_side = left_side.replace("a50\n", "x\n");
    let mut mode = DiffMode::new(
        diff(&left_side, &right_side),
        false,
        false,
        SplitRows::Aligned,
    );
    assert!(is_aligned_split(&mode));
    mode.init(VIEWPORT);
    let before = mode.cursor_location();

    mode.toggle(false, VIEWPORT);
    assert!(mode.is_unified());
    assert_eq!(mode.cursor_location(), before);

    mode.toggle(false, VIEWPORT);
    assert!(is_aligned_split(&mode));
    assert_eq!(mode.cursor_location(), before);
}

#[test]
fn added_files_are_shown_unified_also_with_aligned_rows() {
    let mode = DiffMode::new(diff("", "a\n"), false, true, SplitRows::Aligned);
    assert!(mode.is_unified());

    let changed = diff("a\nb\n", "a\nc\n");
    let mut mode = DiffMode::new(changed.clone(), false, true, SplitRows::Aligned);
    mode.init(VIEWPORT);
    mode.replace_diff(diff("a\nb\n", ""), false, true, VIEWPORT);
    assert!(mode.is_unified());
    mode.replace_diff(changed, false, true, VIEWPORT);
    assert!(is_aligned_split(&mode));
}

#[test]
fn a_pointer_target_that_does_not_fit_the_diff_mode_is_ignored() {
    let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let right_side = left_side.replace("a50\n", "x\n");
    let mut mode = DiffMode::new(
        diff(&left_side, &right_side),
        false,
        false,
        SplitRows::Aligned,
    );
    mode.init(VIEWPORT);

    mode.click(PointerTarget::Row(10));
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 10));

    mode.click(PointerTarget::Line(Side::Left, 20));
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 10));
}

#[test]
fn fold_at_ignores_a_pointer_target_that_does_not_fit_the_diff_mode() {
    let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let right_side = left_side.replace("a50\n", "x\n");
    let changed = diff(&left_side, &right_side);

    let aligned = DiffMode::new(changed.clone(), false, true, SplitRows::Aligned);
    assert!(aligned.fold_at(PointerTarget::Row(0)).is_some());
    assert_eq!(aligned.fold_at(PointerTarget::Line(Side::Left, 0)), None);

    let synced = DiffMode::new(changed, false, true, SplitRows::Synced);
    assert!(synced.fold_at(PointerTarget::Line(Side::Left, 0)).is_some());
    assert_eq!(synced.fold_at(PointerTarget::Row(0)), None);
}

#[test]
fn an_added_file_that_gets_a_left_side_keeps_the_cursor_in_aligned_split() {
    let right_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
    let mut mode = DiffMode::new(diff("", &right_side), false, false, SplitRows::Aligned);
    mode.init(VIEWPORT);
    mode.click(PointerTarget::Row(30));
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 30));

    let left_side = right_side.replace("a50\n", "x\n");
    mode.replace_diff(diff(&left_side, &right_side), false, false, VIEWPORT);
    assert!(is_aligned_split(&mode));
    assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 30));
}
