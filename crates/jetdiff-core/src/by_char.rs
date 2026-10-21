//! Port of `comparePunctuation` from `ByCharRt.kt`, the only part the word diff needs.

use crate::iterable::ChangeBuilder;
use crate::lcs::{self, Changes, DiffTooBig};
use crate::trim::is_punctuation;

/// Matches punctuation only; everything else stays changed.
pub fn compare_punctuation(text1: &str, text2: &str) -> Result<Changes, DiffTooBig> {
    let chars1 = punctuation_chars(text1);
    let chars2 = punctuation_chars(text2);

    let changes = lcs::diff_ints(&chars1.code_points, &chars2.code_points)?;

    let mut builder = ChangeBuilder::new(text1.len(), text2.len());
    for range in changes.unchanged() {
        for i in 0..range.end1 - range.start1 {
            // Punctuation is always a single character
            builder.mark_equal(
                chars1.offsets[range.start1 + i],
                chars2.offsets[range.start2 + i],
            );
        }
    }
    Ok(builder.finish())
}

struct CodePointsOffsets {
    code_points: Vec<u32>,
    offsets: Vec<usize>,
}

/// Java works per UTF-16 unit. Punctuation is ASCII, so bytes give the same matches.
fn punctuation_chars(text: &str) -> CodePointsOffsets {
    let (offsets, code_points) = text
        .bytes()
        .enumerate()
        .filter(|&(_, b)| is_punctuation(u32::from(b)))
        .map(|(i, b)| (i, u32::from(b)))
        .unzip();
    CodePointsOffsets {
        code_points,
        offsets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcs::Range;

    #[test]
    fn punctuation_matches_alone() {
        let c = compare_punctuation("a.b,", "x.y;").unwrap();
        assert_eq!(
            c.unchanged().collect::<Vec<_>>(),
            vec![Range::new(1, 2, 1, 2)]
        );
    }
}
