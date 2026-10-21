//! Port of `LineFragmentSplitter.kt`. Splits a word diff into blocks of lines, split on
//! matched newlines and matched first words in the line.

use crate::by_word::InlineChunk;
use crate::lcs::{Changes, Range};
use crate::policy::{ComparisonPolicy, is_equals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WordBlock {
    pub(crate) words: Range,
    pub(crate) offsets: Range,
}

struct PendingChunk {
    block: WordBlock,
    has_equal_words: bool,
    has_words_inside: bool,
    is_equal_ignore_whitespaces: bool,
}

pub(crate) struct LineFragmentSplitter<'a, 't> {
    text1: &'a str,
    text2: &'a str,
    words1: &'a [InlineChunk<'t>],
    words2: &'a [InlineChunk<'t>],
    iterable: &'a Changes,
    result: Vec<WordBlock>,
    // -1 is the start of the text, words.len() the end, everything between is indices in words
    last1: isize,
    last2: isize,
    pending_chunk: Option<PendingChunk>,
}

impl<'a, 't> LineFragmentSplitter<'a, 't> {
    pub(crate) fn new(
        text1: &'a str,
        text2: &'a str,
        words1: &'a [InlineChunk<'t>],
        words2: &'a [InlineChunk<'t>],
        iterable: &'a Changes,
    ) -> Self {
        Self {
            text1,
            text2,
            words1,
            words2,
            iterable,
            result: Vec::new(),
            last1: -1,
            last2: -1,
            pending_chunk: None,
        }
    }

    pub(crate) fn run(mut self) -> Vec<WordBlock> {
        let mut has_equal_words = false;
        for range in self.iterable.unchanged() {
            for i in 0..range.end1 - range.start1 {
                let index1 = range.start1 + i;
                let index2 = range.start2 + i;

                if self.words1[index1].is_newline() && self.words2[index2].is_newline() {
                    self.add_line_chunk(index1 as isize, index2 as isize, has_equal_words);
                    has_equal_words = false;
                } else {
                    if is_first_in_line(self.words1, index1)
                        && is_first_in_line(self.words2, index2)
                    {
                        self.add_line_chunk(
                            index1 as isize - 1,
                            index2 as isize - 1,
                            has_equal_words,
                        );
                    }
                    has_equal_words = true;
                }
            }
        }
        self.add_line_chunk(
            self.words1.len() as isize,
            self.words2.len() as isize,
            has_equal_words,
        );

        if let Some(pending) = self.pending_chunk.take() {
            self.result.push(pending.block);
        }
        self.result
    }

    fn add_line_chunk(&mut self, end1: isize, end2: isize, has_equal_words: bool) {
        if self.last1 > end1 || self.last2 > end2 {
            return;
        }

        let chunk = self.create_chunk(self.last1, self.last2, end1, end2, has_equal_words);
        if chunk.block.offsets.is_empty() {
            return;
        }

        self.pending_chunk = Some(match self.pending_chunk.take() {
            Some(pending) if should_merge_chunks(&pending, &chunk) => {
                merge_chunks(&pending, &chunk)
            }
            Some(pending) => {
                self.result.push(pending.block);
                chunk
            }
            None => chunk,
        });

        self.last1 = end1;
        self.last2 = end2;
    }

    fn create_chunk(
        &self,
        start1: isize,
        start2: isize,
        end1: isize,
        end2: isize,
        has_equal_words: bool,
    ) -> PendingChunk {
        let start_offset1 = get_offset(self.words1, self.text1, start1);
        let start_offset2 = get_offset(self.words2, self.text2, start2);
        let end_offset1 = get_offset(self.words1, self.text1, end1);
        let end_offset2 = get_offset(self.words2, self.text2, end2);

        let word_start1 = (start1 + 1).max(0) as usize;
        let word_start2 = (start2 + 1).max(0) as usize;
        let word_end1 = ((end1 + 1) as usize).min(self.words1.len());
        let word_end2 = ((end2 + 1) as usize).min(self.words2.len());

        let block = WordBlock {
            words: Range::new(word_start1, word_end1, word_start2, word_end2),
            offsets: Range::new(start_offset1, end_offset1, start_offset2, end_offset2),
        };

        PendingChunk {
            has_equal_words,
            has_words_inside: self.has_words_inside(&block),
            is_equal_ignore_whitespaces: self.is_equals_ignore_whitespace(&block),
            block,
        }
    }

    fn is_equals_ignore_whitespace(&self, block: &WordBlock) -> bool {
        let o = block.offsets;
        is_equals(
            &self.text1[o.start1..o.end1],
            &self.text2[o.start2..o.end2],
            ComparisonPolicy::IgnoreWhitespaces,
        )
    }

    fn has_words_inside(&self, block: &WordBlock) -> bool {
        let w = block.words;
        self.words1[w.start1..w.end1]
            .iter()
            .chain(&self.words2[w.start2..w.end2])
            .any(|word| !word.is_newline())
    }
}

fn should_merge_chunks(chunk1: &PendingChunk, chunk2: &PendingChunk) -> bool {
    // Lines that only matched on '\n'
    if !chunk1.has_equal_words && !chunk2.has_equal_words {
        return true;
    }
    // Lines with whitespace-only changes
    if chunk1.is_equal_ignore_whitespaces && chunk2.is_equal_ignore_whitespaces {
        return true;
    }
    // A block with no words in it
    !chunk1.has_words_inside || !chunk2.has_words_inside
}

fn merge_chunks(chunk1: &PendingChunk, chunk2: &PendingChunk) -> PendingChunk {
    let (b1, b2) = (chunk1.block, chunk2.block);
    PendingChunk {
        block: WordBlock {
            words: Range::new(
                b1.words.start1,
                b2.words.end1,
                b1.words.start2,
                b2.words.end2,
            ),
            offsets: Range::new(
                b1.offsets.start1,
                b2.offsets.end1,
                b1.offsets.start2,
                b2.offsets.end2,
            ),
        },
        has_equal_words: chunk1.has_equal_words || chunk2.has_equal_words,
        has_words_inside: chunk1.has_words_inside || chunk2.has_words_inside,
        is_equal_ignore_whitespaces: chunk1.is_equal_ignore_whitespaces
            && chunk2.is_equal_ignore_whitespaces,
    }
}

fn get_offset(words: &[InlineChunk], text: &str, index: isize) -> usize {
    if index == -1 {
        return 0;
    }
    let index = index as usize;
    if index == words.len() {
        return text.len();
    }
    let chunk = &words[index];
    assert!(chunk.is_newline());
    chunk.offset2()
}

fn is_first_in_line(words: &[InlineChunk], index: usize) -> bool {
    index == 0 || words[index - 1].is_newline()
}
