//! Port av `ByCharRt.kt`. Diffen går på kodepunkter som i Java, offsets er UTF-8 bytes.
//! Java teller UTF-16-enheter per kodepunkt, vi teller `len_utf8`. Begge summerer til hele tegn,
//! så grensene havner på samme tegn.

use crate::change_corrector::ChangeCorrector;
use crate::iterable::ChangeBuilder;
use crate::lcs::{self, Changes, DiffTooBig};
use crate::trim::{is_punctuation, is_white_space_code_point};

pub fn compare(text1: &str, text2: &str) -> Result<Changes, DiffTooBig> {
    let code_points1 = all_code_points(text1);
    let code_points2 = all_code_points(text2);

    let iterable = lcs::diff_ints(&code_points1, &code_points2)?;

    let (mut offset1, mut offset2) = (0, 0);
    let mut builder = ChangeBuilder::new(text1.len(), text2.len());
    for (range, equals) in iterable.iter_all() {
        let end1 = offset1 + count_bytes(&code_points1[range.start1..range.end1]);
        let end2 = offset2 + count_bytes(&code_points2[range.start2..range.end2]);

        if equals {
            builder.mark_equal_range(offset1, offset2, end1, end2);
        }

        offset1 = end1;
        offset2 = end2;
    }
    assert_eq!(offset1, text1.len());
    assert_eq!(offset2, text2.len());

    Ok(builder.finish())
}

pub fn compare_two_step(text1: &str, text2: &str) -> Result<Changes, DiffTooBig> {
    let code_points1 = non_space_code_points(text1);
    let code_points2 = non_space_code_points(text2);

    let non_space_changes = lcs::diff_ints(&code_points1.code_points, &code_points2.code_points)?;
    DefaultCharChangeCorrector {
        code_points1: &code_points1,
        code_points2: &code_points2,
        text1,
        text2,
    }
    .build(&non_space_changes)
}

/// Matcher bare tegnsetting, alt annet blir stående som endret.
pub fn compare_punctuation(text1: &str, text2: &str) -> Result<Changes, DiffTooBig> {
    let chars1 = punctuation_chars(text1);
    let chars2 = punctuation_chars(text2);

    let changes = lcs::diff_ints(&chars1.code_points, &chars2.code_points)?;

    let mut builder = ChangeBuilder::new(text1.len(), text2.len());
    for range in changes.unchanged() {
        for i in 0..range.end1 - range.start1 {
            // Tegnsetting er alltid ett tegn
            builder.mark_equal(
                chars1.offsets[range.start1 + i],
                chars2.offsets[range.start2 + i],
            );
        }
    }
    Ok(builder.finish())
}

pub(crate) struct CodePointsOffsets {
    pub(crate) code_points: Vec<u32>,
    pub(crate) offsets: Vec<usize>,
}

impl CodePointsOffsets {
    pub(crate) fn char_offset(&self, index: usize) -> usize {
        self.offsets[index]
    }

    pub(crate) fn char_offset_after(&self, index: usize) -> usize {
        self.offsets[index] + utf8_len(self.code_points[index])
    }
}

/// Fyller gapene mellom matchede ikke-mellomrom-tegn med full tegndiff.
pub(crate) struct DefaultCharChangeCorrector<'a> {
    pub(crate) code_points1: &'a CodePointsOffsets,
    pub(crate) code_points2: &'a CodePointsOffsets,
    pub(crate) text1: &'a str,
    pub(crate) text2: &'a str,
}

impl ChangeCorrector for DefaultCharChangeCorrector<'_> {
    fn lengths(&self) -> (usize, usize) {
        (self.text1.len(), self.text2.len())
    }

    fn match_gap(
        &self,
        builder: &mut ChangeBuilder,
        start1: usize,
        end1: usize,
        start2: usize,
        end2: usize,
    ) -> Result<(), DiffTooBig> {
        let inner_changes = compare(&self.text1[start1..end1], &self.text2[start2..end2])?;
        for chunk in inner_changes.unchanged() {
            builder.mark_equal_count(
                start1 + chunk.start1,
                start2 + chunk.start2,
                chunk.end1 - chunk.start1,
            );
        }
        Ok(())
    }

    fn original_range1(&self, index: usize) -> (usize, usize) {
        (
            self.code_points1.char_offset(index),
            self.code_points1.char_offset_after(index),
        )
    }

    fn original_range2(&self, index: usize) -> (usize, usize) {
        (
            self.code_points2.char_offset(index),
            self.code_points2.char_offset_after(index),
        )
    }
}

fn all_code_points(text: &str) -> Vec<u32> {
    text.chars().map(|c| c as u32).collect()
}

fn non_space_code_points(text: &str) -> CodePointsOffsets {
    let (offsets, code_points) = text
        .char_indices()
        .filter(|&(_, c)| !is_white_space_code_point(c))
        .map(|(i, c)| (i, c as u32))
        .unzip();
    CodePointsOffsets {
        code_points,
        offsets,
    }
}

/// Java går per UTF-16-enhet. Tegnsetting er ASCII, så bytes gir de samme treffene.
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

fn utf8_len(code_point: u32) -> usize {
    char::from_u32(code_point).map_or(1, char::len_utf8)
}

fn count_bytes(code_points: &[u32]) -> usize {
    code_points.iter().map(|&c| utf8_len(c)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcs::Range;

    fn changes(c: &Changes) -> Vec<(usize, usize, usize, usize)> {
        c.ranges
            .iter()
            .map(|r| (r.start1, r.end1, r.start2, r.end2))
            .collect()
    }

    #[test]
    fn tegndiff_bruker_bytes() {
        let c = compare("aæb", "aøb").unwrap();
        assert_eq!(changes(&c), vec![(1, 3, 1, 3)]);
        let c = compare("a😀", "a").unwrap();
        assert_eq!(changes(&c), vec![(1, 5, 1, 1)]);
    }

    #[test]
    fn to_steg_matcher_mellomrom_i_gapene() {
        let c = compare_two_step("x y", "x zy").unwrap();
        assert_eq!(changes(&c), vec![(2, 2, 2, 3)]);
    }

    #[test]
    fn tegnsetting_matches_alene() {
        let c = compare_punctuation("a.b,", "x.y;").unwrap();
        assert_eq!(
            c.unchanged().collect::<Vec<_>>(),
            vec![Range::new(1, 2, 1, 2)]
        );
    }
}
