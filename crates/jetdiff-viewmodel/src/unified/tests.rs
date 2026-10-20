use super::*;
use jetdiff_core::ComparisonPolicy;

const LINE_HEIGHT: f32 = 10.;
/// 20 hele linjer.
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

/// Endringer på linjene 10, 40 og 90 av 100, én linje hver.
fn three_changes() -> Rc<FileDiff> {
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
fn slettet_over_ny() {
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
    assert_eq!(diff.block_rows().len(), 1);
    assert_eq!(diff.block_rows()[0], 1..3);
    assert_eq!(diff.text(1), "b");
    assert_eq!(diff.text(2), "B");
    assert_eq!(diff.row(1).diff_type(), DiffType::Deleted);
    assert_eq!(diff.row(2).diff_type(), DiffType::Inserted);
}

#[test]
fn linje_til_rad() {
    let diff = unified("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(diff.row_of(Side::Left, 1), 1);
    assert_eq!(diff.row_of(Side::Right, 1), 2);
    // Uendret linje på venstre side har samme rad som på høyre
    assert_eq!(diff.row_of(Side::Left, 2), 3);
    assert_eq!(diff.row_of(Side::Right, 2), 3);
}

#[test]
fn ny_fil_har_bare_nye_numre() {
    let diff = unified("", "a\nb\n");
    assert!(diff.rows().iter().all(|row| row.lines[0].is_none()));
    assert_eq!(diff.row_count(), 3);
    let diff = unified("a\nb\n", "");
    assert!(diff.rows().iter().all(|row| row.lines[1].is_none()));
    assert_eq!(diff.text(0), "a");
}

#[test]
fn endret_blokk_gir_to_markorer() {
    let diff = unified("a\nb\nc\n", "a\nB\nc\n");
    let markers = diff.stripe_markers(LINE_HEIGHT);
    let types: Vec<_> = markers.iter().map(|marker| marker.diff_type).collect();
    assert_eq!(types, [DiffType::Deleted, DiffType::Inserted]);
    assert_eq!(markers[0].start_y, 10.);
    assert_eq!(markers[1].start_y, 20.);
}

#[test]
fn apnes_pa_forste_endring() {
    let state = state(three_changes());
    // Linje 10 slettet og ny: rad 10 og 11
    assert_eq!(state.row, 10);
    assert_eq!(state.cursor_block(), Some(0));
}

#[test]
fn n_og_p_lander_pa_starten_av_blokken() {
    let mut state = state(three_changes());
    assert!(run(&mut state, Command::NextChange).animate);
    // Rad 40 for den slettede, pluss raden for ny linje 10
    assert_eq!(state.row, 41);
    run(&mut state, Command::NextChange);
    assert_eq!(state.row, 92);
    assert!(!run(&mut state, Command::NextChange).changed);
    run(&mut state, Command::PrevChange);
    assert_eq!(state.row, 41);
    // Inne i blokken går p til forrige blokk, ikke starten av denne
    run(&mut state, Command::MoveDown);
    run(&mut state, Command::PrevChange);
    assert_eq!(state.row, 10);
}

#[test]
fn j_k_og_g_holder_scrolloff() {
    let mut state = state(file_diff(&numbered("a", 200), &numbered("a", 200)));
    for _ in 0..30 {
        run(&mut state, Command::MoveDown);
    }
    assert_eq!(state.row, 30);
    // 8 rader under cursoren i 20 rader høyt vindu
    assert_eq!(state.scroll_y(), (30. + 1. + 8.) * LINE_HEIGHT - 200.);
    run(&mut state, Command::GoToBottom);
    assert_eq!(state.row, 200);
    assert!(!run(&mut state, Command::MoveDown).changed);
    run(&mut state, Command::GoToTop);
    assert_eq!((state.row, state.scroll_y()), (0, 0.));
}

#[test]
fn slettet_rad_gir_linja_under_pa_hoyre_side() {
    let mut state = state(three_changes());
    assert_eq!(state.location(), Cursor::new(Side::Right, 10));
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 10));
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 11));
}

#[test]
fn bytte_beholder_linja_og_hoyden() {
    let diff = three_changes();
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
fn slettet_rad_gir_venstre_side_i_split() {
    let diff = three_changes();
    let state = state(diff.clone());
    let split = state.to_split(Rc::new(Folding::none(&diff)), VIEWPORT);
    assert_eq!(split.cursor, Cursor::new(Side::Left, 10));
    let back = UnifiedState::from_split(&split, VIEWPORT);
    assert_eq!(back.row, 10);
}

#[test]
fn hjul_dytter_cursoren() {
    let mut state = state(file_diff(&numbered("a", 200), &numbered("a", 200)));
    assert!(state.wheel(100., VIEWPORT));
    // Scrolloff 8 under toppen
    assert_eq!(state.row, 18);
    // Litt tilbake: cursoren har fortsatt scrolloff
    assert!(!state.wheel(-5., VIEWPORT));
    assert_eq!(state.row, 18);
}

#[test]
fn ny_tekst_beholder_linja() {
    let mut state = state(three_changes());
    run(&mut state, Command::NextChange);
    run(&mut state, Command::MoveDown);
    assert_eq!(state.location(), Cursor::new(Side::Right, 40));
    // Endringen på linje 10 er borte, så radene flytter seg
    let old = numbered("a", 100);
    let new = old.replace("a40\n", "x40\n");
    state.replace_diff(file_diff(&old, &new), VIEWPORT);
    assert_eq!(state.location(), Cursor::new(Side::Right, 40));
}
