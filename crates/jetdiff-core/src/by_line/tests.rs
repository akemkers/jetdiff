use super::*;

fn ranges(changes: &Changes) -> Vec<(usize, usize, usize, usize)> {
    changes
        .ranges
        .iter()
        .map(|r| (r.start1, r.end1, r.start2, r.end2))
        .collect()
}

fn lines<'t>(xs: &[&'t str], policy: ComparisonPolicy) -> Vec<Line<'t>> {
    get_lines(xs, policy)
}

/// Runs the IW step like `do_compare`, then step 2 on the result. Returns the IW and final changes.
#[allow(clippy::type_complexity)]
fn second_step(
    a: &[&str],
    b: &[&str],
) -> (
    Vec<(usize, usize, usize, usize)>,
    Vec<(usize, usize, usize, usize)>,
) {
    let l1 = lines(a, ComparisonPolicy::Default);
    let l2 = lines(b, ComparisonPolicy::Default);
    let iw1 = convert_mode(&l1, ComparisonPolicy::IgnoreWhitespaces);
    let iw2 = convert_mode(&l2, ComparisonPolicy::IgnoreWhitespaces);
    let iw = lcs::diff(&iw1, &iw2).unwrap();
    (
        ranges(&iw),
        ranges(&correct_changes_second_step(&l1, &l2, &iw)),
    )
}

#[test]
fn second_step_picks_best_alignment() {
    // The example from IntelliJ, with spaces for dots
    let a = [" {", "  {", "   {"];
    let b = ["  {", "   {"];
    let (iw, result) = second_step(&a, &b);
    assert_eq!(iw, vec![(2, 3, 2, 2)]);
    assert_eq!(result, vec![(0, 1, 0, 0)]);
}

#[test]
fn second_step_keeps_iw_matched_lines() {
    // Blocks of equal size: no search, pair in order and keep the ones that are exactly equal
    let a = [" {", "  {", "   {"];
    let b = ["   {", "  {", " {"];
    let (iw, result) = second_step(&a, &b);
    assert_eq!(iw, vec![]);
    assert_eq!(result, vec![(0, 1, 0, 1), (2, 3, 2, 3)]);
}

#[test]
fn second_step_keeps_equal_lines() {
    let a = ["a", " b", "c"];
    let b = ["a", "b", "c"];
    assert_eq!(second_step(&a, &b).1, vec![(1, 2, 1, 2)]);
}

#[test]
fn alignment_brute_force() {
    let l1 = lines(&["x", "y"], ComparisonPolicy::Default);
    let l2 = lines(&["q", "x", "q", "y", "q"], ComparisonPolicy::Default);
    assert_eq!(
        best_matching_alignment(&[0, 1], &[0, 1, 2, 3, 4], &l1, &l2),
        vec![1, 3]
    );

    // No equal pairs: first combination
    let l2 = lines(&["q", "q", "q"], ComparisonPolicy::Default);
    assert_eq!(
        best_matching_alignment(&[0, 1], &[0, 1, 2], &l1, &l2),
        vec![0, 1]
    );

    // The first one wins on a tie
    let l1 = lines(&["x"], ComparisonPolicy::Default);
    let l2 = lines(&["x", "x"], ComparisonPolicy::Default);
    assert_eq!(best_matching_alignment(&[0], &[0, 1], &l1, &l2), vec![0]);
}

#[test]
fn alignment_is_skipped_for_large_blocks() {
    // 11 IW-equal lines against 1: over the limit, so first pairs with first and nothing becomes equal
    let mut a = vec![" x"; 11];
    a[5] = "x";
    let b = ["x"];
    assert_eq!(second_step(&a, &b).1, vec![(0, 11, 0, 1)]);

    // Under the limit the exact match is found
    let a = [" x", "x", " x"];
    assert_eq!(second_step(&a, &b).1, vec![(0, 1, 0, 0), (2, 3, 1, 1)]);
}

#[test]
fn non_space_chars_counts_utf16() {
    let line = Line::new(" 😀 é\t", ComparisonPolicy::Default);
    assert_eq!(line.non_space_chars, 3);
}

#[test]
fn line_equality_by_policy() {
    let eq = |a: &str, b: &str, p| Line::new(a, p) == Line::new(b, p);
    assert!(!eq("a b", "a  b", ComparisonPolicy::Default));
    assert!(eq(" a b ", "a b", ComparisonPolicy::TrimWhitespaces));
    assert!(!eq("a b", "ab", ComparisonPolicy::TrimWhitespaces));
    assert!(eq(" a\tb ", "ab", ComparisonPolicy::IgnoreWhitespaces));
}

#[test]
fn compare_policies() {
    let a = ["fun main() {", "    println()", "}"];
    let b = ["fun main() {", "  println() ", "}"];
    assert_eq!(
        ranges(&compare(&a, &b, ComparisonPolicy::Default).unwrap()),
        vec![(1, 2, 1, 2)]
    );
    assert_eq!(
        ranges(&compare(&a, &b, ComparisonPolicy::TrimWhitespaces).unwrap()),
        vec![]
    );
    assert_eq!(
        ranges(&compare(&a, &b, ComparisonPolicy::IgnoreWhitespaces).unwrap()),
        vec![]
    );
}
