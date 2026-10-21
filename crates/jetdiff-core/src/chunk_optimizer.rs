//! Port of `ChunkOptimizer.kt`. Shifts pure insert/delete blocks along equal elements
//! to merge changes and put the boundaries in "good" places.
//!
//! The generic part is `optimize`; the strategy is `get_shift`.

use crate::by_line::Line;
use crate::by_word::InlineChunk;
use crate::lcs::{Changes, Range};
use crate::policy::{UNIMPORTANT_LINE_CHAR_COUNT, is_ws_byte};
use crate::side::Side;
use crate::trim::{expand_backward, expand_forward};

/// `get_shift(touch_side, equal_forward, equal_backward, range1, range2)` returns 0 for no shift,
/// > 0 to shift forward and < 0 to shift backward.
pub(crate) fn optimize<T: Eq>(
    data1: &[T],
    data2: &[T],
    changes: &Changes,
    mut get_shift: impl FnMut(Side, usize, usize, Range, Range) -> isize,
) -> Changes {
    let mut ranges: Vec<Range> = Vec::new();
    for range in changes.unchanged() {
        ranges.push(range);
        process_last_ranges(data1, data2, &mut ranges, &mut get_shift);
    }
    Changes::from_unchanged(&ranges, data1.len(), data2.len())
}

fn process_last_ranges<T: Eq>(
    data1: &[T],
    data2: &[T],
    ranges: &mut Vec<Range>,
    get_shift: &mut impl FnMut(Side, usize, usize, Range, Range) -> isize,
) {
    // A loop instead of the recursion in the original; merging tries again with the previous pair
    while ranges.len() >= 2 {
        let range1 = ranges[ranges.len() - 2];
        let range2 = ranges[ranges.len() - 1];
        if range1.end1 != range2.start1 && range1.end2 != range2.start2 {
            // Not a pure insert/delete. Would only be shiftable if the input were not an LCS.
            return;
        }

        let count1 = range1.end1 - range1.start1;
        let count2 = range2.end1 - range2.start1;

        let equal_forward = expand_forward(
            data1,
            data2,
            range1.end1,
            range1.end2,
            range1.end1 + count2,
            range1.end2 + count2,
        );
        let equal_backward = expand_backward(
            data1,
            data2,
            range2.start1 - count1,
            range2.start2 - count1,
            range2.start1,
            range2.start2,
        );

        if equal_forward == 0 && equal_backward == 0 {
            return;
        }

        // [A]B[B] -> [AB]B
        if equal_forward == count2 {
            ranges.truncate(ranges.len() - 2);
            ranges.push(Range::new(
                range1.start1,
                range1.end1 + count2,
                range1.start2,
                range1.end2 + count2,
            ));
            continue;
        }

        // [A]A[B] -> A[AB]
        if equal_backward == count1 {
            ranges.truncate(ranges.len() - 2);
            ranges.push(Range::new(
                range2.start1 - count1,
                range2.end1,
                range2.start2 - count1,
                range2.end2,
            ));
            continue;
        }

        let touch_side = Side::from_left(range1.end1 == range2.start1);

        let shift = get_shift(touch_side, equal_forward, equal_backward, range1, range2);
        if shift != 0 {
            let s = |x: usize| {
                x.checked_add_signed(shift)
                    .expect("shift outside the sequence")
            };
            ranges.truncate(ranges.len() - 2);
            ranges.push(Range::new(
                range1.start1,
                s(range1.end1),
                range1.start2,
                s(range1.end2),
            ));
            ranges.push(Range::new(
                s(range2.start1),
                range2.end1,
                s(range2.start2),
                range2.end2,
            ));
        }
        return;
    }
}

/// `WordChunkOptimizer`: prefer boundaries where the words are separated by whitespace.
pub(crate) fn optimize_word_chunks(
    text1: &str,
    text2: &str,
    words1: &[InlineChunk],
    words2: &[InlineChunk],
    changes: &Changes,
) -> Changes {
    optimize(
        words1,
        words2,
        changes,
        |touch_side, eq_forward, eq_backward, _, range2| {
            let touch_words = touch_side.select(words1, words2);
            let touch_text = touch_side.select(text1, text2);
            let touch_start = touch_side.select(range2.start1, range2.start2);

            // Already separated by whitespace
            if is_separated_with_whitespace(
                touch_text,
                &touch_words[touch_start - 1],
                &touch_words[touch_start],
            ) {
                return 0;
            }

            // [X]A Y[A ZA] -> [XA] YA [ZA]
            if let Some(left_shift) =
                find_sequence_edge_shift(touch_text, touch_words, touch_start, eq_forward, true)
            {
                return left_shift as isize;
            }

            // [AX A]Y A[Z] -> [AX] AY [AZ]
            if let Some(right_shift) = find_sequence_edge_shift(
                touch_text,
                touch_words,
                touch_start - 1,
                eq_backward,
                false,
            ) {
                return -(right_shift as isize);
            }

            0
        },
    )
}

fn find_sequence_edge_shift(
    text: &str,
    words: &[InlineChunk],
    offset: usize,
    count: usize,
    left_to_right: bool,
) -> Option<usize> {
    (0..count)
        .find(|&i| {
            let (word1, word2) = if left_to_right {
                (&words[offset + i], &words[offset + i + 1])
            } else {
                (&words[offset - i - 1], &words[offset - i])
            };
            is_separated_with_whitespace(text, word1, word2)
        })
        .map(|i| i + 1)
}

fn is_separated_with_whitespace(text: &str, word1: &InlineChunk, word2: &InlineChunk) -> bool {
    if word1.is_newline() || word2.is_newline() {
        return true;
    }
    text.as_bytes()[word1.offset2()..word2.offset1()]
        .iter()
        .any(|&b| is_ws_byte(b))
}

/// `LineChunkOptimizer`: prefer insert/delete bounded by empty (or "unimportant") lines.
pub(crate) fn optimize_line_chunks(lines1: &[Line], lines2: &[Line], changes: &Changes) -> Changes {
    let optimizer = LineChunkOptimizer { lines1, lines2 };
    optimize(
        lines1,
        lines2,
        changes,
        |touch_side, eq_forward, eq_backward, range1, range2| {
            optimizer.get_shift(touch_side, eq_forward, eq_backward, range1, range2)
        },
    )
}

struct LineChunkOptimizer<'a, 't> {
    lines1: &'a [Line<'t>],
    lines2: &'a [Line<'t>],
}

impl LineChunkOptimizer<'_, '_> {
    fn get_shift(
        &self,
        touch_side: Side,
        equal_forward: usize,
        equal_backward: usize,
        range1: Range,
        range2: Range,
    ) -> isize {
        let args = (touch_side, equal_forward, equal_backward, range1, range2);
        self.unchanged_boundary_shift(args, 0)
            .or_else(|| self.changed_boundary_shift(args, 0))
            .or_else(|| self.unchanged_boundary_shift(args, UNIMPORTANT_LINE_CHAR_COUNT))
            .or_else(|| self.changed_boundary_shift(args, UNIMPORTANT_LINE_CHAR_COUNT))
            .unwrap_or(0)
    }

    /// Empty line in unchanged context: the insert/delete should sit right before or after it.
    fn unchanged_boundary_shift(&self, args: ShiftArgs, threshold: usize) -> Option<isize> {
        let (touch_side, equal_forward, equal_backward, _, range2) = args;
        let touch_lines = touch_side.select(self.lines1, self.lines2);
        let touch_start = touch_side.select(range2.start1, range2.start2);

        let shift_forward =
            find_next_unimportant_line(touch_lines, touch_start, equal_forward + 1, threshold);
        let shift_backward =
            find_prev_unimportant_line(touch_lines, touch_start - 1, equal_backward + 1, threshold);

        pick_shift(shift_forward, shift_backward)
    }

    /// Empty line in the changed block: the insert/delete should start or end with it.
    fn changed_boundary_shift(&self, args: ShiftArgs, threshold: usize) -> Option<isize> {
        let (touch_side, equal_forward, equal_backward, range1, range2) = args;
        let non_touch_side = touch_side.other();
        let non_touch_lines = non_touch_side.select(self.lines1, self.lines2);
        let change_start = non_touch_side.select(range1.end1, range1.end2);
        let change_end = non_touch_side.select(range2.start1, range2.start2);

        let shift_forward =
            find_next_unimportant_line(non_touch_lines, change_start, equal_forward + 1, threshold);
        let shift_backward = find_prev_unimportant_line(
            non_touch_lines,
            change_end - 1,
            equal_backward + 1,
            threshold,
        );

        pick_shift(shift_forward, shift_backward)
    }
}

type ShiftArgs = (Side, usize, usize, Range, Range);

fn find_next_unimportant_line(
    lines: &[Line],
    offset: usize,
    count: usize,
    threshold: usize,
) -> Option<usize> {
    (0..count).find(|&i| lines[offset + i].non_space_chars <= threshold)
}

fn find_prev_unimportant_line(
    lines: &[Line],
    offset: usize,
    count: usize,
    threshold: usize,
) -> Option<usize> {
    (0..count).find(|&i| lines[offset - i].non_space_chars <= threshold)
}

/// `None` means try the next rule. If we are already on a good boundary, we do not shift.
fn pick_shift(shift_forward: Option<usize>, shift_backward: Option<usize>) -> Option<isize> {
    match (shift_forward, shift_backward) {
        (None, None) => None,
        (Some(0), _) | (_, Some(0)) => Some(0),
        (Some(f), _) => Some(f as isize),
        (None, Some(b)) => Some(-(b as isize)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ComparisonPolicy;

    /// "" is an empty line, the rest are ordinary lines. Changes in, changes out.
    fn run(
        a: &[&str],
        b: &[&str],
        changes: &[(usize, usize, usize, usize)],
    ) -> Vec<(usize, usize, usize, usize)> {
        fn lines<'t>(xs: &[&'t str]) -> Vec<Line<'t>> {
            xs.iter()
                .map(|x| Line::new(x, ComparisonPolicy::Default))
                .collect()
        }
        let (lines1, lines2) = (lines(a), lines(b));
        let changes = Changes {
            len1: a.len(),
            len2: b.len(),
            ranges: changes
                .iter()
                .map(|&(s1, e1, s2, e2)| Range::new(s1, e1, s2, e2))
                .collect(),
        };
        optimize_line_chunks(&lines1, &lines2, &changes)
            .ranges
            .iter()
            .map(|r| (r.start1, r.end1, r.start2, r.end2))
            .collect()
    }

    #[test]
    fn already_at_empty_line_is_not_shifted() {
        let a = ["A", "", "B"];
        let b = ["A", "", "X", "", "B"];
        assert_eq!(run(&a, &b, &[(1, 1, 1, 3)]), vec![(1, 1, 1, 3)]);
    }

    #[test]
    fn shifts_forward_to_empty_line_in_context() {
        let a = ["P", "Q", "", "R"];
        let b = ["P", "Q", "X", "Q", "", "R"];
        assert_eq!(run(&a, &b, &[(1, 1, 1, 3)]), vec![(2, 2, 2, 4)]);
    }

    #[test]
    fn shifts_backward_to_empty_line_in_context() {
        let a = ["R", "", "Q", "P"];
        let b = ["R", "", "Q", "X", "Q", "P"];
        assert_eq!(run(&a, &b, &[(3, 3, 3, 5)]), vec![(2, 2, 2, 4)]);
    }

    #[test]
    fn inserted_block_starts_with_empty_line() {
        let a = ["P", "Q", "R"];
        let b = ["P", "Q", "", "X", "Q", "R"];
        assert_eq!(run(&a, &b, &[(1, 1, 1, 4)]), vec![(2, 2, 2, 5)]);
    }

    #[test]
    fn unimportant_line_is_used_when_none_are_empty() {
        // Lines with more than 3 characters are "big", so only "}" is unimportant
        let a = ["pppp", "qqqq", "}", "rrrr"];
        let b = ["pppp", "qqqq", "xxxx", "qqqq", "}", "rrrr"];
        assert_eq!(run(&a, &b, &[(1, 1, 1, 3)]), vec![(2, 2, 2, 4)]);
    }

    #[test]
    fn merges_with_next_change() {
        // [A]B[B] -> [AB]B
        let a = ["P", "Q", "Z"];
        let b = ["P", "Q", "Q", "W"];
        assert_eq!(
            run(&a, &b, &[(1, 1, 1, 2), (2, 3, 3, 4)]),
            vec![(2, 3, 2, 4)]
        );
    }

    #[test]
    fn merges_with_previous_change() {
        // [A]A[B] -> A[AB]
        let a = ["Z", "Q", "P"];
        let b = ["W", "Q", "Q", "P"];
        assert_eq!(
            run(&a, &b, &[(0, 1, 0, 1), (2, 2, 2, 3)]),
            vec![(0, 1, 0, 2)]
        );
    }
}
