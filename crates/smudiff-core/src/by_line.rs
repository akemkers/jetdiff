//! Port av `ByLineRt.kt` (bare 2-veis). Linjer matches først whitespace-blindt, og
//! etterpå strammes matchingen inn til valgt policy.

use std::hash::{Hash, Hasher};

use crate::change_corrector::{ChangeCorrector, SmartLineChangeCorrector};
use crate::chunk_optimizer::optimize_line_chunks;
use crate::iterable::{ChangeBuilder, expand_change_builder};
use crate::lcs::{self, Changes, DiffTooBig, Range};
use crate::policy::{
    ComparisonPolicy, UNIMPORTANT_LINE_CHAR_COUNT, hash_code, is_equals, is_space_enter_or_tab,
};
use crate::trim::expand;

/// Linjer uten `'\n'`. Resultatet er fair.
pub fn compare(
    lines1: &[&str],
    lines2: &[&str],
    policy: ComparisonPolicy,
) -> Result<Changes, DiffTooBig> {
    do_compare(
        &get_lines(lines1, policy),
        &get_lines(lines2, policy),
        policy,
    )
}

fn do_compare(
    lines1: &[Line],
    lines2: &[Line],
    policy: ComparisonPolicy,
) -> Result<Changes, DiffTooBig> {
    if policy == ComparisonPolicy::IgnoreWhitespaces {
        let changes = compare_smart(lines1, lines2)?;
        let changes = optimize_line_chunks(lines1, lines2, &changes);
        Ok(expand_ranges(lines1, lines2, &changes))
    } else {
        let iw_lines1 = convert_mode(lines1, ComparisonPolicy::IgnoreWhitespaces);
        let iw_lines2 = convert_mode(lines2, ComparisonPolicy::IgnoreWhitespaces);

        let iw_changes = compare_smart(&iw_lines1, &iw_lines2)?;
        // Med vilje original-policy her, som i IntelliJ
        let iw_changes = optimize_line_chunks(lines1, lines2, &iw_changes);
        Ok(correct_changes_second_step(lines1, lines2, &iw_changes))
    }
}

/// Gjør IW-matchingen strikt etter policy.
///
/// Linjer som er matchet og like beholdes. Linjer som ikke er matchet forblir umatchet.
/// Linjer som er matchet men ulike: finn alle IW-like linjer i samme umatchede blokk og
/// velg matchingen med flest eksakt like par. Da blir `.{ ..{ ...{` mot `..{ ...{` matchet
/// som (1,3)-(0,2), mens `.{ ..{ ...{` mot `...{ ..{ .{` ikke mister IW-matchede linjer.
fn correct_changes_second_step(lines1: &[Line], lines2: &[Line], changes: &Changes) -> Changes {
    let mut step = SecondStep {
        lines1,
        lines2,
        builder: expand_change_builder(lines1, lines2),
        sample: None,
        last1: 0,
        last2: 0,
    };
    step.run(changes);
    step.builder.finish()
}

struct SecondStep<'a, 't, B> {
    lines1: &'a [Line<'t>],
    lines2: &'a [Line<'t>],
    builder: B,
    sample: Option<&'t str>,
    last1: usize,
    last2: usize,
}

impl<F: Fn(Range) -> Range> SecondStep<'_, '_, ChangeBuilder<F>> {
    fn run(&mut self, changes: &Changes) {
        for range in changes.unchanged() {
            for i in 0..range.end1 - range.start1 {
                let index1 = range.start1 + i;
                let index2 = range.start2 + i;
                let line1 = &self.lines1[index1];
                let line2 = &self.lines2[index2];

                if !self.is_sample(line1.content) {
                    self.flush(index1, index2);
                    if line1 == line2 {
                        self.builder.mark_equal(index1, index2);
                    } else {
                        self.sample = Some(line1.content);
                    }
                }
            }
        }
        self.flush(changes.len1, changes.len2);
    }

    fn is_sample(&self, content: &str) -> bool {
        self.sample
            .is_some_and(|s| is_equals(s, content, ComparisonPolicy::IgnoreWhitespaces))
    }

    fn flush(&mut self, line1: usize, line2: usize) {
        if self.sample.is_none() {
            return;
        }

        let start1 = self.last1.max(self.builder.index1());
        let start2 = self.last2.max(self.builder.index2());

        let mut sub_lines1 = Vec::new();
        let mut sub_lines2 = Vec::new();
        for i in start1..line1 {
            if self.is_sample(self.lines1[i].content) {
                sub_lines1.push(i);
                self.last1 = i + 1;
            }
        }
        for i in start2..line2 {
            if self.is_sample(self.lines2[i].content) {
                sub_lines2.push(i);
                self.last2 = i + 1;
            }
        }

        assert!(!sub_lines1.is_empty() && !sub_lines2.is_empty());
        self.align_exact_matching(&sub_lines1, &sub_lines2);

        self.sample = None;
    }

    fn align_exact_matching(&mut self, sub_lines1: &[usize], sub_lines2: &[usize]) {
        let n = sub_lines1.len().max(sub_lines2.len());
        // Brute force er C(n, k), grensen holder søkerommet på rundt 250 tilfeller
        let skip_aligning = n > 10 || sub_lines1.len() == sub_lines2.len();

        let pairs: Vec<(usize, usize)> = if skip_aligning {
            sub_lines1
                .iter()
                .copied()
                .zip(sub_lines2.iter().copied())
                .collect()
        } else if sub_lines1.len() < sub_lines2.len() {
            let matching =
                best_matching_alignment(sub_lines1, sub_lines2, self.lines1, self.lines2);
            (0..sub_lines1.len())
                .map(|i| (sub_lines1[i], sub_lines2[matching[i]]))
                .collect()
        } else {
            let matching =
                best_matching_alignment(sub_lines2, sub_lines1, self.lines2, self.lines1);
            (0..sub_lines2.len())
                .map(|i| (sub_lines1[matching[i]], sub_lines2[i]))
                .collect()
        };

        for (index1, index2) in pairs {
            if self.lines1[index1] == self.lines2[index2] {
                self.builder.mark_equal(index1, index2);
            }
        }
    }
}

/// For hver linje i `sub_lines1`, indeksen i `sub_lines2` den skal pares med. Velger
/// kombinasjonen med flest like par, første vinner ved likhet.
fn best_matching_alignment(
    sub_lines1: &[usize],
    sub_lines2: &[usize],
    lines1: &[Line],
    lines2: &[Line],
) -> Vec<usize> {
    assert!(sub_lines1.len() < sub_lines2.len());
    let size = sub_lines1.len();

    struct Search<'a, 'b, 't> {
        sub_lines1: &'a [usize],
        sub_lines2: &'a [usize],
        lines1: &'b [Line<'t>],
        lines2: &'b [Line<'t>],
        comb: Vec<usize>,
        best: Vec<usize>,
        best_weight: usize,
    }

    impl Search<'_, '_, '_> {
        fn combinations(&mut self, start: usize, n: usize, k: usize) {
            if k == self.comb.len() {
                self.process_combination();
                return;
            }
            for i in start..=n {
                self.comb[k] = i;
                self.combinations(i + 1, n, k + 1);
            }
        }

        fn process_combination(&mut self) {
            let weight = (0..self.comb.len())
                .filter(|&i| {
                    self.lines1[self.sub_lines1[i]] == self.lines2[self.sub_lines2[self.comb[i]]]
                })
                .count();
            if weight > self.best_weight {
                self.best_weight = weight;
                self.best.copy_from_slice(&self.comb);
            }
        }
    }

    let mut search = Search {
        sub_lines1,
        sub_lines2,
        lines1,
        lines2,
        comb: vec![0; size],
        best: (0..size).collect(),
        best_weight: 0,
    };
    search.combinations(0, sub_lines2.len() - 1, 0);
    search.best
}

/// Sammenlign i to steg: først uten "uviktige" linjer, så diff gapene mellom matchede blokker.
fn compare_smart(lines1: &[Line], lines2: &[Line]) -> Result<Changes, DiffTooBig> {
    let (big_lines1, indexes1) = get_big_lines(lines1, UNIMPORTANT_LINE_CHAR_COUNT);
    let (big_lines2, indexes2) = get_big_lines(lines2, UNIMPORTANT_LINE_CHAR_COUNT);

    let changes = lcs::diff(&big_lines1, &big_lines2)?;
    SmartLineChangeCorrector {
        indexes1: &indexes1,
        indexes2: &indexes2,
        lines1,
        lines2,
    }
    .build(&changes)
}

fn get_big_lines<'t>(lines: &[Line<'t>], threshold: usize) -> (Vec<Line<'t>>, Vec<usize>) {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.non_space_chars > threshold)
        .map(|(i, line)| (line.clone(), i))
        .unzip()
}

fn expand_ranges(lines1: &[Line], lines2: &[Line], changes: &Changes) -> Changes {
    let ranges = changes
        .ranges
        .iter()
        .map(|ch| expand(lines1, lines2, ch.start1, ch.start2, ch.end1, ch.end2))
        .filter(|r| !r.is_empty())
        .collect();
    Changes {
        len1: lines1.len(),
        len2: lines2.len(),
        ranges,
    }
}

fn get_lines<'t>(lines: &[&'t str], policy: ComparisonPolicy) -> Vec<Line<'t>> {
    lines.iter().map(|&line| Line::new(line, policy)).collect()
}

fn convert_mode<'t>(original: &[Line<'t>], policy: ComparisonPolicy) -> Vec<Line<'t>> {
    original
        .iter()
        .map(|line| {
            if line.policy != policy {
                Line::new(line.content, policy)
            } else {
                line.clone()
            }
        })
        .collect()
}

/// En linje med likhet og hash etter policy.
#[derive(Debug, Clone)]
pub(crate) struct Line<'t> {
    pub(crate) content: &'t str,
    policy: ComparisonPolicy,
    hash: i32,
    /// Teller UTF-16-enheter som Java, så tegn utenfor BMP teller dobbelt mot terskelen.
    pub(crate) non_space_chars: usize,
}

impl<'t> Line<'t> {
    pub(crate) fn new(content: &'t str, policy: ComparisonPolicy) -> Self {
        Self {
            content,
            policy,
            hash: hash_code(content, policy),
            non_space_chars: content
                .chars()
                .filter(|&c| !is_space_enter_or_tab(c))
                .map(char::len_utf16)
                .sum(),
        }
    }
}

impl PartialEq for Line<'_> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert_eq!(self.policy, other.policy);
        self.hash == other.hash && is_equals(self.content, other.content, self.policy)
    }
}

impl Eq for Line<'_> {}

impl Hash for Line<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash);
    }
}

#[cfg(test)]
mod tests;
