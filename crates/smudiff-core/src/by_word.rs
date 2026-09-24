//! Port av to-sidedelen av `ByWordRt.kt`. Offsets er UTF-8 bytes. Tokeniseringen går på
//! kodepunkter som i Java, og whitespace og tegnsetting er ASCII, så grensene blir de samme.

use std::hash::{Hash, Hasher};

use crate::by_char;
use crate::chunk_optimizer::optimize_word_chunks;
use crate::fragments::DiffFragment;
use crate::iterable::ChangeBuilder;
use crate::lcs::{self, Changes, DiffTooBig, Range};
use crate::line_fragment_splitter::{LineFragmentSplitter, WordBlock};
use crate::policy::{ComparisonPolicy, is_ws_byte};
use crate::trim::{
    expand_whitespaces, expand_whitespaces_backward, expand_whitespaces_forward, is_alpha,
    is_continuous_script, is_equals_ignore_whitespaces_range, is_equals_range, trim_end,
    trim_range, trim_start,
};

/// Ord eller newline. Mellomrom og tegnsetting blir ikke chunks.
#[derive(Debug, Clone, Copy)]
pub enum InlineChunk<'t> {
    Word {
        offset1: usize,
        offset2: usize,
        hash: i32,
        content: &'t str,
    },
    Newline {
        offset1: usize,
    },
}

impl InlineChunk<'_> {
    pub fn offset1(&self) -> usize {
        match *self {
            InlineChunk::Word { offset1, .. } | InlineChunk::Newline { offset1 } => offset1,
        }
    }

    pub fn offset2(&self) -> usize {
        match *self {
            InlineChunk::Word { offset2, .. } => offset2,
            InlineChunk::Newline { offset1 } => offset1 + 1,
        }
    }

    pub fn is_newline(&self) -> bool {
        matches!(self, InlineChunk::Newline { .. })
    }
}

impl PartialEq for InlineChunk<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                InlineChunk::Word {
                    hash: h1,
                    content: c1,
                    ..
                },
                InlineChunk::Word {
                    hash: h2,
                    content: c2,
                    ..
                },
            ) => h1 == h2 && c1 == c2,
            (InlineChunk::Newline { .. }, InlineChunk::Newline { .. }) => true,
            _ => false,
        }
    }
}

impl Eq for InlineChunk<'_> {}

impl Hash for InlineChunk<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            InlineChunk::Word { hash, .. } => hash.hash(state),
            InlineChunk::Newline { .. } => state.write_u8(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBlock {
    pub fragments: Vec<DiffFragment>,
    pub offsets: Range,
    pub newlines1: usize,
    pub newlines2: usize,
}

pub fn compare_and_split(
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
) -> Result<Vec<LineBlock>, DiffTooBig> {
    let words1 = get_inline_chunks(text1);
    let words2 = get_inline_chunks(text2);

    let word_changes = lcs::diff(&words1, &words2)?;
    let word_changes = optimize_word_chunks(text1, text2, &words1, &words2, &word_changes);

    let word_blocks =
        LineFragmentSplitter::new(text1, text2, &words1, &words2, &word_changes).run();

    let sub_iterables = collect_word_block_sub_iterables(&word_changes, &word_blocks);

    let mut line_blocks = Vec::with_capacity(word_blocks.len());
    for (block, subiterable) in word_blocks.iter().zip(&sub_iterables) {
        let offsets = block.offsets;
        let words = block.words;

        let subtext1 = &text1[offsets.start1..offsets.end1];
        let subtext2 = &text2[offsets.start2..offsets.end2];

        let subwords1 = &words1[words.start1..words.end1];
        let subwords2 = &words2[words.start2..words.end2];

        let delimiters_iterable = match_adjustment_delimiters(
            subtext1,
            subtext2,
            subwords1,
            subwords2,
            subiterable,
            offsets.start1,
            offsets.start2,
        )?;
        let iterable =
            match_adjustment_whitespaces(subtext1, subtext2, &delimiters_iterable, policy);

        line_blocks.push(LineBlock {
            fragments: convert_into_diff_fragments(&iterable),
            offsets,
            newlines1: count_newlines(subwords1),
            newlines2: count_newlines(subwords2),
        });
    }

    Ok(line_blocks)
}

fn collect_word_block_sub_iterables(
    word_changes: &Changes,
    word_blocks: &[WordBlock],
) -> Vec<Changes> {
    let changed = &word_changes.ranges;
    let mut index = 0;

    word_blocks
        .iter()
        .map(|block| {
            let words = block.words;
            while index < changed.len() {
                let range = changed[index];
                if range.end1 < words.start1 || range.end2 < words.start2 {
                    index += 1;
                    continue;
                }
                break;
            }
            Changes::subiterable(
                changed,
                words.start1,
                words.end1,
                words.start2,
                words.end2,
                index,
            )
        })
        .collect()
}

fn match_adjustment_delimiters(
    text1: &str,
    text2: &str,
    words1: &[InlineChunk],
    words2: &[InlineChunk],
    changes: &Changes,
    start_shift1: usize,
    start_shift2: usize,
) -> Result<Changes, DiffTooBig> {
    AdjustmentPunctuationMatcher {
        text1,
        text2,
        words1,
        words2,
        start_shift1,
        start_shift2,
        builder: ChangeBuilder::new(text1.len(), text2.len()),
        last: None,
    }
    .build(changes)
}

fn match_adjustment_whitespaces(
    text1: &str,
    text2: &str,
    iterable: &Changes,
    policy: ComparisonPolicy,
) -> Changes {
    match policy {
        ComparisonPolicy::Default => default_corrector(iterable, text1, text2),
        ComparisonPolicy::TrimWhitespaces => {
            let default_iterable = default_corrector(iterable, text1, text2);
            trim_spaces_corrector(&default_iterable, text1, text2)
        }
        ComparisonPolicy::IgnoreWhitespaces => ignore_spaces_corrector(iterable, text1, text2),
    }
}

pub fn convert_into_diff_fragments(changes: &Changes) -> Vec<DiffFragment> {
    changes
        .ranges
        .iter()
        .map(|ch| DiffFragment {
            offsets1: ch.start1..ch.end1,
            offsets2: ch.start2..ch.end2,
        })
        .collect()
}

/// Sammenligner `text1` mot `text21 + text22` som én tekst, og deler resultatet i to.
fn compare_punctuation_2_side(
    text1: &str,
    text21: &str,
    text22: &str,
) -> Result<(Changes, Changes), DiffTooBig> {
    let text2 = [text21, text22].concat();
    let changes = by_char::compare_punctuation(text1, &text2)?;

    let (ranges1, ranges2) = split_iterable_2_side(&changes, text21.len());

    let iterable1 = Changes::from_unchanged(&ranges1, text1.len(), text21.len());
    let iterable2 = Changes::from_unchanged(&ranges2, text1.len(), text22.len());
    Ok((iterable1, iterable2))
}

fn split_iterable_2_side(changes: &Changes, offset: usize) -> (Vec<Range>, Vec<Range>) {
    let mut ranges1 = Vec::new();
    let mut ranges2 = Vec::new();
    for ch in changes.unchanged() {
        if ch.end2 <= offset {
            ranges1.push(ch);
        } else if ch.start2 >= offset {
            ranges2.push(Range::new(
                ch.start1,
                ch.end1,
                ch.start2 - offset,
                ch.end2 - offset,
            ));
        } else {
            let len2 = offset - ch.start2;
            ranges1.push(Range::new(ch.start1, ch.start1 + len2, ch.start2, offset));
            ranges2.push(Range::new(ch.start1 + len2, ch.end1, 0, ch.end2 - offset));
        }
    }
    (ranges1, ranges2)
}

/// `start` kan være -1 (fra `end - 1` på tom range), da er svaret false.
fn is_leading_trailing_space(text: &str, start: isize) -> bool {
    is_leading_space(text, start) || is_trailing_space(text, start)
}

fn is_leading_space(text: &str, start: isize) -> bool {
    let bytes = text.as_bytes();
    if start < 0 {
        return false;
    }
    let start = start as usize;
    if start == bytes.len() || !is_ws_byte(bytes[start]) {
        return false;
    }

    for &c in bytes[..start].iter().rev() {
        if c == b'\n' {
            return true;
        }
        if !is_ws_byte(c) {
            return false;
        }
    }
    true
}

fn is_trailing_space(text: &str, end: isize) -> bool {
    let bytes = text.as_bytes();
    if end < 0 {
        return false;
    }
    let end = end as usize;
    if end == bytes.len() || !is_ws_byte(bytes[end]) {
        return false;
    }

    for &c in &bytes[end..] {
        if c == b'\n' {
            return true;
        }
        if !is_ws_byte(c) {
            return false;
        }
    }
    true
}

fn count_newlines(words: &[InlineChunk]) -> usize {
    words.iter().filter(|w| w.is_newline()).count()
}

pub fn get_inline_chunks(text: &str) -> Vec<InlineChunk<'_>> {
    let mut chunks = Vec::new();

    let mut word_start: Option<usize> = None;
    let mut word_hash: i32 = 0;

    for (offset, ch) in text.char_indices() {
        let is_alpha = is_alpha(ch);
        let is_word_part = is_alpha && !is_continuous_script(ch);

        if is_word_part {
            if word_start.is_none() {
                word_start = Some(offset);
                word_hash = 0;
            }
            word_hash = word_hash.wrapping_mul(31).wrapping_add(ch as i32);
        } else {
            if let Some(start) = word_start.take() {
                chunks.push(word_chunk(text, start, offset, word_hash));
            }

            if is_alpha {
                // Kontinuerlig skrift, hvert tegn er et eget ord
                chunks.push(word_chunk(text, offset, offset + ch.len_utf8(), ch as i32));
            } else if ch == '\n' {
                chunks.push(InlineChunk::Newline { offset1: offset });
            }
        }
    }

    if let Some(start) = word_start {
        chunks.push(word_chunk(text, start, text.len(), word_hash));
    }

    chunks
}

fn word_chunk(text: &str, offset1: usize, offset2: usize, hash: i32) -> InlineChunk<'_> {
    InlineChunk::Word {
        offset1,
        offset2,
        hash,
        content: &text[offset1..offset2],
    }
}

/// Matcher tegnsetting mellom ordene. Eksempel: "[ X { A ! B } Y ]" mot "( X ... Y )" gir tre
/// grupper: "[" mot "(", "{" + "}" mot "..." og "]" mot ")".
struct AdjustmentPunctuationMatcher<'a, 't> {
    text1: &'a str,
    text2: &'a str,
    words1: &'a [InlineChunk<'t>],
    words2: &'a [InlineChunk<'t>],
    start_shift1: usize,
    start_shift2: usize,
    builder: ChangeBuilder,
    // (start1, start2, end1, end2) fra forrige match_forward
    last: Option<(usize, usize, usize, usize)>,
}

impl AdjustmentPunctuationMatcher<'_, '_> {
    fn build(mut self, changes: &Changes) -> Result<Changes, DiffTooBig> {
        self.execute(changes)?;
        Ok(self.builder.finish())
    }

    fn execute(&mut self, changes: &Changes) -> Result<(), DiffTooBig> {
        self.last = None;

        self.match_forward_index(-1, -1);

        for ch in changes.unchanged() {
            for i in 0..ch.end1 - ch.start1 {
                let index1 = ch.start1 + i;
                let index2 = ch.start2 + i;

                let start1 = self.start_offset1(index1);
                let start2 = self.start_offset2(index2);
                let end1 = self.end_offset1(index1);
                let end2 = self.end_offset2(index2);

                self.match_backward_index(index1, index2)?;

                self.builder.mark_equal_range(start1, start2, end1, end2);

                self.match_forward_index(index1 as isize, index2 as isize);
            }
        }

        self.match_backward_index(self.words1.len(), self.words2.len())
    }

    fn match_backward_index(&mut self, index1: usize, index2: usize) -> Result<(), DiffTooBig> {
        let start1 = if index1 == 0 {
            0
        } else {
            self.end_offset1(index1 - 1)
        };
        let start2 = if index2 == 0 {
            0
        } else {
            self.end_offset2(index2 - 1)
        };
        let end1 = if index1 == self.words1.len() {
            self.text1.len()
        } else {
            self.start_offset1(index1)
        };
        let end2 = if index2 == self.words2.len() {
            self.text2.len()
        } else {
            self.start_offset2(index2)
        };

        self.match_backward(start1, start2, end1, end2)?;
        self.last = None;
        Ok(())
    }

    fn match_forward_index(&mut self, index1: isize, index2: isize) {
        let start1 = if index1 == -1 {
            0
        } else {
            self.end_offset1(index1 as usize)
        };
        let start2 = if index2 == -1 {
            0
        } else {
            self.end_offset2(index2 as usize)
        };
        let next1 = (index1 + 1) as usize;
        let next2 = (index2 + 1) as usize;
        let end1 = if next1 == self.words1.len() {
            self.text1.len()
        } else {
            self.start_offset1(next1)
        };
        let end2 = if next2 == self.words2.len() {
            self.text2.len()
        } else {
            self.start_offset2(next2)
        };

        assert!(self.last.is_none());
        self.last = Some((start1, start2, end1, end2));
    }

    fn match_backward(
        &mut self,
        start1: usize,
        start2: usize,
        end1: usize,
        end2: usize,
    ) -> Result<(), DiffTooBig> {
        let (last_start1, last_start2, last_end1, last_end2) =
            self.last.expect("match_forward mangler");

        if last_start1 == start1 && last_start2 == start2 {
            // Matchede ord rett etter hverandre, match gapet mellom ("A B" - "A B")
            assert!(last_end1 == end1 && last_end2 == end2);
            return self.match_range(start1, start2, end1, end2);
        }
        if last_start1 < start1 && last_start2 < start2 {
            // Umatchede ord mellom på begge sider ("A X B" - "A Y B")
            assert!(last_end1 <= start1 && last_end2 <= start2);
            self.match_range(last_start1, last_start2, last_end1, last_end2)?;
            return self.match_range(start1, start2, end1, end2);
        }

        // Umatchede ord mellom på bare én side ("A B" - "A Y B")
        self.match_complex_range(
            last_start1,
            last_start2,
            last_end1,
            last_end2,
            start1,
            start2,
            end1,
            end2,
        )
    }

    fn match_range(
        &mut self,
        start1: usize,
        start2: usize,
        end1: usize,
        end2: usize,
    ) -> Result<(), DiffTooBig> {
        if start1 == end1 && start2 == end2 {
            return Ok(());
        }

        let changes =
            by_char::compare_punctuation(&self.text1[start1..end1], &self.text2[start2..end2])?;

        for ch in changes.unchanged() {
            self.builder.mark_equal_range(
                start1 + ch.start1,
                start2 + ch.start2,
                start1 + ch.end1,
                start2 + ch.end2,
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn match_complex_range(
        &mut self,
        start11: usize,
        start12: usize,
        end11: usize,
        end12: usize,
        start21: usize,
        start22: usize,
        end21: usize,
        end22: usize,
    ) -> Result<(), DiffTooBig> {
        if start11 == start21 && end11 == end21 {
            self.match_complex_range_left(start11, end11, start12, end12, start22, end22)
        } else if start12 == start22 && end12 == end22 {
            self.match_complex_range_right(start12, end12, start11, end11, start21, end21)
        } else {
            unreachable!("gapene overlapper ikke på noen side")
        }
    }

    fn match_complex_range_left(
        &mut self,
        start1: usize,
        end1: usize,
        start12: usize,
        end12: usize,
        start22: usize,
        end22: usize,
    ) -> Result<(), DiffTooBig> {
        let (first, second) = compare_punctuation_2_side(
            &self.text1[start1..end1],
            &self.text2[start12..end12],
            &self.text2[start22..end22],
        )?;

        for ch in first.unchanged() {
            self.builder.mark_equal_range(
                start1 + ch.start1,
                start12 + ch.start2,
                start1 + ch.end1,
                start12 + ch.end2,
            );
        }
        for ch in second.unchanged() {
            self.builder.mark_equal_range(
                start1 + ch.start1,
                start22 + ch.start2,
                start1 + ch.end1,
                start22 + ch.end2,
            );
        }
        Ok(())
    }

    fn match_complex_range_right(
        &mut self,
        start2: usize,
        end2: usize,
        start11: usize,
        end11: usize,
        start21: usize,
        end21: usize,
    ) -> Result<(), DiffTooBig> {
        let (first, second) = compare_punctuation_2_side(
            &self.text2[start2..end2],
            &self.text1[start11..end11],
            &self.text1[start21..end21],
        )?;

        // Speilet, 2-side-sammenligningen har to høyresider mens vi har to venstresider
        for ch in first.unchanged() {
            self.builder.mark_equal_range(
                start11 + ch.start2,
                start2 + ch.start1,
                start11 + ch.end2,
                start2 + ch.end1,
            );
        }
        for ch in second.unchanged() {
            self.builder.mark_equal_range(
                start21 + ch.start2,
                start2 + ch.start1,
                start21 + ch.end2,
                start2 + ch.end1,
            );
        }
        Ok(())
    }

    fn start_offset1(&self, index: usize) -> usize {
        self.words1[index].offset1() - self.start_shift1
    }

    fn start_offset2(&self, index: usize) -> usize {
        self.words2[index].offset1() - self.start_shift2
    }

    fn end_offset1(&self, index: usize) -> usize {
        self.words1[index].offset2() - self.start_shift1
    }

    fn end_offset2(&self, index: usize) -> usize {
        self.words2[index].offset2() - self.start_shift2
    }
}

fn default_corrector(iterable: &Changes, text1: &str, text2: &str) -> Changes {
    let mut changes = Vec::new();
    for range in &iterable.ranges {
        let end_cut = expand_whitespaces_backward(
            text1,
            text2,
            range.start1,
            range.start2,
            range.end1,
            range.end2,
        );
        let start_cut = expand_whitespaces_forward(
            text1,
            text2,
            range.start1,
            range.start2,
            range.end1 - end_cut,
            range.end2 - end_cut,
        );

        let expand = Range::new(
            range.start1 + start_cut,
            range.end1 - end_cut,
            range.start2 + start_cut,
            range.end2 - end_cut,
        );

        if !expand.is_empty() {
            changes.push(expand);
        }
    }
    Changes {
        len1: text1.len(),
        len2: text2.len(),
        ranges: changes,
    }
}

fn ignore_spaces_corrector(iterable: &Changes, text1: &str, text2: &str) -> Changes {
    let mut changes = Vec::new();
    for &range in &iterable.ranges {
        // Match mellomrom der vi kan, ignorer dem der vi ikke kan
        let expanded = expand_whitespaces(text1, text2, range);
        let trimmed = trim_range(
            text1,
            text2,
            expanded.start1,
            expanded.start2,
            expanded.end1,
            expanded.end2,
        );

        if !trimmed.is_empty() && !is_equals_ignore_whitespaces_range(text1, text2, trimmed) {
            changes.push(trimmed);
        }
    }
    Changes {
        len1: text1.len(),
        len2: text2.len(),
        ranges: changes,
    }
}

fn trim_spaces_corrector(iterable: &Changes, text1: &str, text2: &str) -> Changes {
    let mut changes = Vec::new();
    for range in &iterable.ranges {
        let (mut start1, mut start2, mut end1, mut end2) =
            (range.start1, range.start2, range.end1, range.end2);

        if is_leading_trailing_space(text1, start1 as isize) {
            start1 = trim_start(text1, start1, end1);
        }
        if is_leading_trailing_space(text1, end1 as isize - 1) {
            end1 = trim_end(text1, start1, end1);
        }
        if is_leading_trailing_space(text2, start2 as isize) {
            start2 = trim_start(text2, start2, end2);
        }
        if is_leading_trailing_space(text2, end2 as isize - 1) {
            end2 = trim_end(text2, start2, end2);
        }

        let trimmed = Range::new(start1, end1, start2, end2);

        if !trimmed.is_empty() && !is_equals_range(text1, text2, trimmed) {
            changes.push(trimmed);
        }
    }
    Changes {
        len1: text1.len(),
        len2: text2.len(),
        ranges: changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(text: &str) -> Vec<&str> {
        get_inline_chunks(text)
            .iter()
            .map(|c| match c {
                InlineChunk::Word { content, .. } => *content,
                InlineChunk::Newline { .. } => "\\n",
            })
            .collect()
    }

    fn fragments<'a>(
        text1: &'a str,
        text2: &'a str,
        policy: ComparisonPolicy,
    ) -> Vec<(&'a str, &'a str)> {
        compare_and_split(text1, text2, policy)
            .unwrap()
            .into_iter()
            .flat_map(|block| {
                let (s1, s2) = (block.offsets.start1, block.offsets.start2);
                block.fragments.into_iter().map(move |f| {
                    (
                        &text1[s1 + f.offsets1.start..s1 + f.offsets1.end],
                        &text2[s2 + f.offsets2.start..s2 + f.offsets2.end],
                    )
                })
            })
            .collect()
    }

    #[test]
    fn tokenisering() {
        assert_eq!(
            chunks("foo_bar, baz\nqux"),
            ["foo_bar", "baz", "\\n", "qux"]
        );
        assert_eq!(chunks("ab中文cd"), ["ab", "中", "文", "cd"]);
        assert_eq!(chunks("x→y"), ["x", "→", "y"]);
        assert_eq!(chunks("æøå١٢"), ["æøå١٢"]);
    }

    #[test]
    fn hash_er_lik_java() {
        let InlineChunk::Word { hash, .. } = get_inline_chunks("hello")[0] else {
            panic!()
        };
        // "hello".hashCode() i Java
        assert_eq!(hash, 99162322);
    }

    #[test]
    fn ord_og_tegnsetting() {
        let d = ComparisonPolicy::Default;
        assert_eq!(fragments("foo bar baz", "foo qux baz", d), [("bar", "qux")]);
        assert_eq!(fragments("f(a, b)", "f(a)", d), [(", b", "")]);
        assert_eq!(
            fragments("[ X { A ! B } Y ]", "( X ... Y )", d),
            [("[", "("), ("{ A ! B }", "..."), ("]", ")")]
        );
    }

    #[test]
    fn whitespace_policyer() {
        assert_eq!(
            fragments("a  b", "a b", ComparisonPolicy::Default),
            [(" ", "")]
        );
        assert!(fragments("a  b", "a b", ComparisonPolicy::IgnoreWhitespaces).is_empty());
        assert!(fragments("  a", "a", ComparisonPolicy::TrimWhitespaces).is_empty());
        assert_eq!(
            fragments("a  b", "a b", ComparisonPolicy::TrimWhitespaces),
            [(" ", "")]
        );
    }

    #[test]
    fn deler_i_linjeblokker() {
        let blocks =
            compare_and_split("a x\nb\nc y\n", "a z\nb\nc w\n", ComparisonPolicy::Default).unwrap();
        let offsets: Vec<_> = blocks.iter().map(|b| b.offsets).collect();
        assert_eq!(
            offsets,
            [
                Range::new(0, 4, 0, 4),
                Range::new(4, 6, 4, 6),
                Range::new(6, 10, 6, 10)
            ]
        );
        assert_eq!(blocks[0].newlines1, 1);
        assert!(blocks[1].fragments.is_empty());
    }
}
