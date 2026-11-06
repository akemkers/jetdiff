//! Diff model for one file. Corresponds to what `SimpleDiffViewer` builds from `SimpleDiffChange`
//! and the highlighters from `SimpleDiffChangeUi`. Columns are UTF-8 bytes within the line.

use std::ops::Range;

use jetdiff_core::side::Side;
use jetdiff_core::{ComparisonPolicy, DiffFragment, DiffTooBig, LineOffsets, compare_for_viewer};
use jetdiff_syntax::{Language, Token};

use crate::columns::display_width;
use crate::scroll::LineMapping;

/// `TextDiffType` without CONFLICT, which is only used in merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HunkKind {
    Added,
    Deleted,
    Modified,
}

impl HunkKind {
    /// `DiffUtil.getDiffType(hasDeleted, hasInserted)`.
    fn from_sides(has_deleted: bool, has_added: bool) -> Self {
        match (has_deleted, has_added) {
            (true, false) => HunkKind::Deleted,
            (false, true) => HunkKind::Added,
            // Both empty is logged as an error in IntelliJ and becomes MODIFIED
            _ => HunkKind::Modified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub lines: [Range<usize>; 2],
    /// Absolute byte offsets for whole lines, as in `LineFragment`.
    pub offsets: [Range<usize>; 2],
    pub kind: HunkKind,
    /// `None` when the hunk was not compared by word. `Some(vec![])` was compared by word and
    /// found no words that differ.
    pub inline_highlights: Option<Vec<DiffFragment>>,
}

impl Hunk {
    pub fn lines(&self, side: Side) -> Range<usize> {
        self.lines[idx(side)].clone()
    }

    /// Such a hunk gets a fainter background, so its inline highlights stand out
    /// (`SimpleDiffChangeUi.createHighlighter`: `ignored = innerFragments != null`).
    pub fn compared_by_word(&self) -> bool {
        self.inline_highlights.is_some()
    }
}

/// Inline highlight in full color for its own kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineHighlight {
    /// An empty range is the empty side of an inline highlight: a 2px vertical stroke at
    /// `cols.start` (`DiffEmptyHighlighterRenderer`).
    pub cols: Range<usize>,
    pub kind: HunkKind,
    /// The highlight covers the line break, so the color continues to the right edge of the viewport.
    pub past_line_end: bool,
}

/// Syntax token within one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub cols: Range<usize>,
    pub token: Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineHighlight<'a> {
    /// The hunk the line is in.
    pub hunk: Option<usize>,
    pub inline: &'a [InlineHighlight],
}

#[derive(Debug, Clone)]
struct SideData {
    text: String,
    line_offsets: LineOffsets,
    /// Hunk per line, for lookup without searching.
    line_hunk: Vec<Option<usize>>,
    /// Inline highlights per line, flat and sorted by line.
    inline: Vec<InlineHighlight>,
    inline_start: Vec<usize>,
    /// Empty without a language. Same lookup as `inline`.
    syntax: Vec<SyntaxSpan>,
    syntax_start: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    sides: [SideData; 2],
    hunks: Vec<Hunk>,
    mapping: LineMapping,
}

impl FileDiff {
    pub fn new(
        left_side_text: &str,
        right_side_text: &str,
        policy: ComparisonPolicy,
    ) -> Result<Self, DiffTooBig> {
        let hunks = compare_for_viewer(left_side_text, right_side_text, policy)?
            .into_iter()
            .map(|f| Hunk {
                // `DiffUtil.getLineDiffType`
                kind: HunkKind::from_sides(!f.lines1.is_empty(), !f.lines2.is_empty()),
                lines: [f.lines1, f.lines2],
                offsets: [f.offsets1, f.offsets2],
                inline_highlights: f.inner,
            })
            .collect::<Vec<_>>();
        let sides = [
            SideData::new(left_side_text, &hunks, Side::Left),
            SideData::new(right_side_text, &hunks, Side::Right),
        ];
        let mapping = LineMapping::new(&hunks, [sides[0].line_count(), sides[1].line_count()]);
        Ok(Self {
            sides,
            hunks,
            mapping,
        })
    }

    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    /// The hunk the line is in. Deleted lines exist only on Left and added lines only
    /// on Right. On the other side the line right below where they are missing applies.
    pub fn hunk_at(&self, side: Side, line: usize) -> Option<usize> {
        self.side(side)
            .line_hunk
            .get(line)
            .copied()
            .flatten()
            .or_else(|| {
                self.hunks
                    .iter()
                    .position(|hunk| hunk.lines(side).is_empty() && hunk.lines(side).start == line)
            })
    }

    pub fn mapping(&self) -> &LineMapping {
        &self.mapping
    }

    pub fn text(&self, side: Side) -> &str {
        &self.side(side).text
    }

    /// The side shown alone when the other is empty, as for added and deleted files.
    pub fn single_side(&self) -> Option<Side> {
        if self.text(Side::Left).is_empty() {
            Some(Side::Right)
        } else if self.text(Side::Right).is_empty() {
            Some(Side::Left)
        } else {
            None
        }
    }

    /// Added and deleted files are always shown Unified, so Split doesn't have to show one side alone.
    pub fn can_split(&self) -> bool {
        self.single_side().is_none()
    }

    /// The lines of a side that are in hunks: deleted on Left, added on Right. A Modified hunk
    /// counts on both.
    pub fn changed_line_count(&self, side: Side) -> usize {
        self.hunks.iter().map(|hunk| hunk.lines(side).len()).sum()
    }

    /// At least one line. Text that ends with `'\n'` has an empty last line, as in IntelliJ.
    pub fn line_count(&self, side: Side) -> usize {
        self.side(side).line_count()
    }

    /// The line the byte offset is on.
    pub fn line_of_offset(&self, side: Side, offset: usize) -> usize {
        line_of_offset(&self.side(side).line_offsets, offset)
    }

    /// Widest line on both sides, in columns.
    pub fn max_display_width(&self) -> usize {
        [Side::Left, Side::Right]
            .into_iter()
            .flat_map(|side| {
                (0..self.line_count(side)).map(move |line| display_width(self.line(side, line)))
            })
            .max()
            .unwrap_or(0)
    }

    /// Without `'\n'`.
    pub fn line(&self, side: Side, line: usize) -> &str {
        let s = self.side(side);
        &s.text[s.line_offsets.line_start(line)..s.line_offsets.line_end(line)]
    }

    pub fn line_highlight(&self, side: Side, line: usize) -> LineHighlight<'_> {
        let s = self.side(side);
        let Some(&hunk) = s.line_hunk.get(line) else {
            return LineHighlight::default();
        };
        LineHighlight {
            hunk,
            inline: &s.inline[s.inline_start[line]..s.inline_start[line + 1]],
        }
    }

    /// Parses the whole text on each side, so deleted lines also get the right syntax.
    pub fn highlight_syntax(&mut self, lang: Language) {
        for s in &mut self.sides {
            s.highlight_syntax(lang);
        }
    }

    pub fn line_syntax(&self, side: Side, line: usize) -> &[SyntaxSpan] {
        let s = self.side(side);
        match (s.syntax_start.get(line), s.syntax_start.get(line + 1)) {
            (Some(&a), Some(&b)) => &s.syntax[a..b],
            _ => &[],
        }
    }

    fn side(&self, side: Side) -> &SideData {
        &self.sides[idx(side)]
    }
}

impl SideData {
    fn new(text: &str, hunks: &[Hunk], side: Side) -> Self {
        let line_offsets = LineOffsets::new(text);
        let count = line_offsets.line_count();
        let mut line_hunk = vec![None; count];
        let mut per_line: Vec<Vec<InlineHighlight>> = vec![Vec::new(); count];

        for (i, hunk) in hunks.iter().enumerate() {
            for line in hunk.lines(side) {
                line_hunk[line] = Some(i);
            }
            let base = hunk.offsets[idx(side)].start;
            for highlight in hunk.inline_highlights.iter().flatten() {
                let range = side.select(&highlight.offsets1, &highlight.offsets2);
                let kind = HunkKind::from_sides(
                    !highlight.offsets1.is_empty(),
                    !highlight.offsets2.is_empty(),
                );
                split_into_lines(
                    &line_offsets,
                    base + range.start..base + range.end,
                    kind,
                    &mut per_line,
                );
            }
        }

        let mut inline = Vec::new();
        let mut inline_start = Vec::with_capacity(count + 1);
        for mut line in per_line {
            inline_start.push(inline.len());
            inline.append(&mut line);
        }
        inline_start.push(inline.len());

        Self {
            text: text.to_owned(),
            line_offsets,
            line_hunk,
            inline,
            inline_start,
            syntax: Vec::new(),
            syntax_start: Vec::new(),
        }
    }

    fn highlight_syntax(&mut self, lang: Language) {
        let lo = &self.line_offsets;
        let count = lo.line_count();
        let mut syntax = Vec::new();
        let mut syntax_start = Vec::with_capacity(count + 1);
        let mut spans = jetdiff_syntax::highlight(lang, &self.text)
            .into_iter()
            .peekable();
        for line in 0..count {
            syntax_start.push(syntax.len());
            let (start, end) = (lo.line_start(line), lo.line_end(line));
            let last = line + 1 == count;
            // Spans over several lines (block comments) stay until the last line
            while let Some(span) = spans.peek() {
                if span.range.start >= end && !last {
                    break;
                }
                let cols =
                    span.range.start.max(start) - start..span.range.end.min(end).max(start) - start;
                if !cols.is_empty() {
                    syntax.push(SyntaxSpan {
                        cols,
                        token: span.token,
                    });
                }
                if span.range.end > end && !last {
                    break;
                }
                spans.next();
            }
        }
        syntax_start.push(syntax.len());
        self.syntax = syntax;
        self.syntax_start = syntax_start;
    }

    fn line_count(&self) -> usize {
        self.line_offsets.line_count()
    }
}

/// EXACT_RANGE highlighter split into lines. After the line end, the background is painted for
/// the highlighter that covers the line break, but not one that starts on it
/// (`IterationState.getPastLineEndBackgroundAttributes` and the `atBreak` check in `setAttributes`).
fn split_into_lines(
    lo: &LineOffsets,
    range: Range<usize>,
    kind: HunkKind,
    out: &mut [Vec<InlineHighlight>],
) {
    if range.is_empty() {
        // `offsetToLogicalPosition`: an offset right after '\n' belongs to the next line
        let line = line_of_offset(lo, range.start);
        let col = range.start - lo.line_start(line);
        out[line].push(InlineHighlight {
            cols: col..col,
            kind,
            past_line_end: false,
        });
        return;
    }
    let first = line_of_offset(lo, range.start);
    let last = line_of_offset(lo, range.end - 1);
    for (line, highlights) in out.iter_mut().enumerate().take(last + 1).skip(first) {
        let (start, end) = (lo.line_start(line), lo.line_end(line));
        let cols = range.start.max(start) - start..range.end.min(end).max(start) - start;
        let past_line_end = range.start < end && range.end > end;
        if cols.is_empty() && !past_line_end {
            continue;
        }
        highlights.push(InlineHighlight {
            cols,
            kind,
            past_line_end,
        });
    }
}

fn line_of_offset(lo: &LineOffsets, offset: usize) -> usize {
    let (mut low, mut high) = (0, lo.line_count() - 1);
    while low < high {
        let mid = (low + high).div_ceil(2);
        if lo.line_start(mid) <= offset {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

pub(crate) fn idx(side: Side) -> usize {
    side.select(0, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    #[test]
    fn kind_from_empty_sides() {
        let d = diff("a\nb\n", "a\nx\nb\n");
        assert_eq!(d.hunks()[0].kind, HunkKind::Added);
        let d = diff("a\nx\nb\n", "a\nb\n");
        assert_eq!(d.hunks()[0].kind, HunkKind::Deleted);
        let d = diff("a\nb\n", "a\nc\n");
        assert_eq!(d.hunks()[0].kind, HunkKind::Modified);
    }

    #[test]
    fn hunk_at_line() {
        let d = diff("a\nb\nc\n", "a\nx\nc\n");
        assert_eq!(d.hunk_at(Side::Right, 1), Some(0));
        assert_eq!(d.hunk_at(Side::Right, 0), None);
        // Deleted line: empty on Right, hit on the line after
        let d = diff("a\nx\nb\n", "a\nb\n");
        assert_eq!(d.hunk_at(Side::Left, 1), Some(0));
        assert_eq!(d.hunk_at(Side::Right, 1), Some(0));
        assert_eq!(d.hunk_at(Side::Right, 0), None);
    }

    #[test]
    fn changed_lines_per_side() {
        // A Modified hunk of one line against two, and an added line
        let d = diff("a\nb\nc\n", "a\nx\ny\nc\nd\n");
        assert_eq!(d.changed_line_count(Side::Left), 1);
        assert_eq!(d.changed_line_count(Side::Right), 3);
    }

    #[test]
    fn lines_per_side() {
        let d = diff("a\nb", "a\nb\n");
        assert_eq!(d.line_count(Side::Left), 2);
        assert_eq!(d.line_count(Side::Right), 3);
        assert_eq!(d.line(Side::Left, 1), "b");
        assert_eq!(d.line(Side::Right, 2), "");
    }

    #[test]
    fn line_outside_hunk_has_no_highlight() {
        let d = diff("a\nb\n", "a\nc\n");
        assert_eq!(d.line_highlight(Side::Left, 0), LineHighlight::default());
        assert_eq!(d.line_highlight(Side::Left, 99), LineHighlight::default());
    }

    #[test]
    fn syntax_per_line() {
        let mut d = diff("fn a() {}\n", "/* x\ny */ fn b() {}\n");
        d.highlight_syntax(Language::Rust);
        let tokens = |side, line| -> Vec<_> {
            d.line_syntax(side, line)
                .iter()
                .map(|s| (s.cols.clone(), s.token))
                .collect()
        };
        assert_eq!(tokens(Side::Left, 0)[0], (0..2, Token::Keyword));
        assert_eq!(tokens(Side::Right, 0), vec![(0..4, Token::Comment)]);
        assert_eq!(
            tokens(Side::Right, 1)[..2],
            [(0..4, Token::Comment), (5..7, Token::Keyword)]
        );
        assert!(tokens(Side::Right, 2).is_empty());
        assert!(d.line_syntax(Side::Right, 99).is_empty());
    }

    #[test]
    fn line_of_offset_at_boundaries() {
        let lo = LineOffsets::new("ab\n\ncd");
        assert_eq!(line_of_offset(&lo, 0), 0);
        assert_eq!(line_of_offset(&lo, 2), 0);
        assert_eq!(line_of_offset(&lo, 3), 1);
        assert_eq!(line_of_offset(&lo, 4), 2);
        assert_eq!(line_of_offset(&lo, 6), 2);
    }

    #[test]
    fn inline_highlight_over_line_break_is_split() {
        let lo = LineOffsets::new("ab\ncd\nef");
        let mut out = vec![Vec::new(); 3];
        split_into_lines(&lo, 1..7, HunkKind::Modified, &mut out);
        let h = |cols: Range<usize>, past_line_end| InlineHighlight {
            cols,
            kind: HunkKind::Modified,
            past_line_end,
        };
        assert_eq!(out[0], vec![h(1..2, true)]);
        assert_eq!(out[1], vec![h(0..2, true)]);
        assert_eq!(out[2], vec![h(0..1, false)]);
    }

    #[test]
    fn inline_highlight_starting_on_line_break_is_not_extended() {
        let lo = LineOffsets::new("ab\ncd");
        let mut out = vec![Vec::new(); 2];
        split_into_lines(&lo, 2..4, HunkKind::Added, &mut out);
        assert!(out[0].is_empty());
        assert_eq!(out[1][0].cols, 0..1);
    }

    #[test]
    fn empty_inline_highlight_after_line_break_lands_on_next_line() {
        let lo = LineOffsets::new("ab\ncd");
        let mut out = vec![Vec::new(); 2];
        split_into_lines(&lo, 3..3, HunkKind::Deleted, &mut out);
        assert!(out[0].is_empty());
        assert_eq!(out[1][0].cols, 0..0);
    }
}
