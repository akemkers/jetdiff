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

/// Kjører IW-steget som `do_compare`, så steg 2 på resultatet. Returnerer IW- og endelige endringer.
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
fn second_step_velger_beste_alignment() {
    // Eksempelet fra IntelliJ med mellomrom for prikker
    let a = [" {", "  {", "   {"];
    let b = ["  {", "   {"];
    let (iw, result) = second_step(&a, &b);
    assert_eq!(iw, vec![(2, 3, 2, 2)]);
    assert_eq!(result, vec![(0, 1, 0, 0)]);
}

#[test]
fn second_step_mister_ikke_iw_matchede_linjer() {
    // Like store blokker: ingen søk, par i rekkefølge og behold de som er eksakt like
    let a = [" {", "  {", "   {"];
    let b = ["   {", "  {", " {"];
    let (iw, result) = second_step(&a, &b);
    assert_eq!(iw, vec![]);
    assert_eq!(result, vec![(0, 1, 0, 1), (2, 3, 2, 3)]);
}

#[test]
fn second_step_like_linjer_beholdes() {
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

    // Ingen like par: første kombinasjon
    let l2 = lines(&["q", "q", "q"], ComparisonPolicy::Default);
    assert_eq!(
        best_matching_alignment(&[0, 1], &[0, 1, 2], &l1, &l2),
        vec![0, 1]
    );

    // Første vinner ved likhet
    let l1 = lines(&["x"], ComparisonPolicy::Default);
    let l2 = lines(&["x", "x"], ComparisonPolicy::Default);
    assert_eq!(best_matching_alignment(&[0], &[0, 1], &l1, &l2), vec![0]);
}

#[test]
fn alignment_hoppes_over_for_store_blokker() {
    // 11 IW-like linjer mot 1: over grensen, så første pares med første og ingenting blir likt
    let mut a = vec![" x"; 11];
    a[5] = "x";
    let b = ["x"];
    assert_eq!(second_step(&a, &b).1, vec![(0, 11, 0, 1)]);

    // Under grensen finnes den eksakte matchen
    let a = [" x", "x", " x"];
    assert_eq!(second_step(&a, &b).1, vec![(0, 1, 0, 0), (2, 3, 1, 1)]);
}

#[test]
fn non_space_chars_teller_utf16() {
    let line = Line::new(" 😀 é\t", ComparisonPolicy::Default);
    assert_eq!(line.non_space_chars, 3);
}

#[test]
fn line_likhet_etter_policy() {
    let eq = |a: &str, b: &str, p| Line::new(a, p) == Line::new(b, p);
    assert!(!eq("a b", "a  b", ComparisonPolicy::Default));
    assert!(eq(" a b ", "a b", ComparisonPolicy::TrimWhitespaces));
    assert!(!eq("a b", "ab", ComparisonPolicy::TrimWhitespaces));
    assert!(eq(" a\tb ", "ab", ComparisonPolicy::IgnoreWhitespaces));
}

#[test]
fn compare_policyer() {
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
