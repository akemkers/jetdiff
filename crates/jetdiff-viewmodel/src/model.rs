//! Diffmodell for én fil. Tilsvarer det `SimpleDiffViewer` bygger av `SimpleDiffChange`
//! og highlighterne fra `SimpleDiffChangeUi`. Kolonner er UTF-8 bytes innen linja.

use std::ops::Range;

use jetdiff_core::side::Side;
use jetdiff_core::{ComparisonPolicy, DiffFragment, DiffTooBig, LineOffsets, compare_for_viewer};
use jetdiff_syntax::{Language, Token};

use crate::columns::display_width;
use crate::scroll::LineMapping;

/// `TextDiffType` uten CONFLICT, som bare brukes i merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffType {
    Inserted,
    Deleted,
    Modified,
}

impl DiffType {
    /// `DiffUtil.getDiffType(hasDeleted, hasInserted)`.
    fn from_sides(has_deleted: bool, has_inserted: bool) -> Self {
        match (has_deleted, has_inserted) {
            (true, false) => DiffType::Deleted,
            (false, true) => DiffType::Inserted,
            // Begge tomme logges som feil i IntelliJ og blir MODIFIED
            _ => DiffType::Modified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub lines: [Range<usize>; 2],
    /// Absolutte byte-offsets for hele linjer, som i `LineFragment`.
    pub offsets: [Range<usize>; 2],
    pub diff_type: DiffType,
    /// `None` er hel-linje-highlight i full farge. `Some(vec![])` er ignored-farge uten ord.
    pub inner: Option<Vec<DiffFragment>>,
}

impl Block {
    pub fn lines(&self, side: Side) -> Range<usize> {
        self.lines[idx(side)].clone()
    }

    /// `SimpleDiffChangeUi.createHighlighter`: `ignored = innerFragments != null`.
    pub fn ignored(&self) -> bool {
        self.inner.is_some()
    }
}

/// Ord-highlight i full farge for sin egen type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineHighlight {
    /// Tom range er en tom side av et ord-fragment: 2px vertikal strek ved `cols.start`
    /// (`DiffEmptyHighlighterRenderer`).
    pub cols: Range<usize>,
    pub diff_type: DiffType,
    /// Fragmentet dekker linjeskiftet, så fargen fortsetter til høyre kant av viewporten.
    pub past_line_end: bool,
}

/// Syntaks-token innen én linje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub cols: Range<usize>,
    pub token: Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineHighlight<'a> {
    /// Blokken linja ligger i.
    pub block: Option<usize>,
    pub inline: &'a [InlineHighlight],
}

#[derive(Debug, Clone)]
struct SideData {
    text: String,
    line_offsets: LineOffsets,
    /// Blokk per linje, for oppslag uten søk.
    line_block: Vec<Option<usize>>,
    /// Ord-highlights per linje, flatt og sortert på linje.
    inline: Vec<InlineHighlight>,
    inline_start: Vec<usize>,
    /// Tomt uten språk. Samme oppslag som `inline`.
    syntax: Vec<SyntaxSpan>,
    syntax_start: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    sides: [SideData; 2],
    blocks: Vec<Block>,
    mapping: LineMapping,
}

impl FileDiff {
    pub fn new(text1: &str, text2: &str, policy: ComparisonPolicy) -> Result<Self, DiffTooBig> {
        let fragments = compare_for_viewer(text1, text2, policy)?;
        let blocks = fragments
            .into_iter()
            .map(|f| Block {
                // `DiffUtil.getLineDiffType`
                diff_type: DiffType::from_sides(!f.lines1.is_empty(), !f.lines2.is_empty()),
                lines: [f.lines1, f.lines2],
                offsets: [f.offsets1, f.offsets2],
                inner: f.inner,
            })
            .collect::<Vec<_>>();
        let sides = [
            SideData::new(text1, &blocks, Side::Left),
            SideData::new(text2, &blocks, Side::Right),
        ];
        let mapping = LineMapping::new(&blocks, [sides[0].line_count(), sides[1].line_count()]);
        Ok(Self {
            sides,
            blocks,
            mapping,
        })
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Blokken linja ligger i. Slettede linjer finnes bare på venstre side og nye linjer bare
    /// på høyre. På den andre siden gjelder linja rett under der de mangler.
    pub fn block_at(&self, side: Side, line: usize) -> Option<usize> {
        self.side(side)
            .line_block
            .get(line)
            .copied()
            .flatten()
            .or_else(|| {
                self.blocks
                    .iter()
                    .position(|b| b.lines(side).is_empty() && b.lines(side).start == line)
            })
    }

    pub fn mapping(&self) -> &LineMapping {
        &self.mapping
    }

    pub fn text(&self, side: Side) -> &str {
        &self.side(side).text
    }

    /// Siden som vises alene når den andre er tom, som for nye og slettede filer.
    pub fn single_side(&self) -> Option<Side> {
        if self.text(Side::Left).is_empty() {
            Some(Side::Right)
        } else if self.text(Side::Right).is_empty() {
            Some(Side::Left)
        } else {
            None
        }
    }

    /// Nye og slettede filer vises alltid unified, så split slipper å vise én side alene.
    pub fn can_split(&self) -> bool {
        self.single_side().is_none()
    }

    /// Minst én linje. Tekst som slutter på `'\n'` har en tom siste linje, som i IntelliJ.
    pub fn line_count(&self, side: Side) -> usize {
        self.side(side).line_count()
    }

    /// Linja byte-offsetet står på.
    pub fn line_of_offset(&self, side: Side, offset: usize) -> usize {
        line_of_offset(&self.side(side).line_offsets, offset)
    }

    /// Bredeste linje på begge sider, i kolonner.
    pub fn max_display_width(&self) -> usize {
        [Side::Left, Side::Right]
            .into_iter()
            .flat_map(|side| {
                (0..self.line_count(side)).map(move |line| display_width(self.line(side, line)))
            })
            .max()
            .unwrap_or(0)
    }

    /// Uten `'\n'`.
    pub fn line(&self, side: Side, line: usize) -> &str {
        let s = self.side(side);
        &s.text[s.line_offsets.line_start(line)..s.line_offsets.line_end(line)]
    }

    pub fn line_highlight(&self, side: Side, line: usize) -> LineHighlight<'_> {
        let s = self.side(side);
        let Some(&block) = s.line_block.get(line) else {
            return LineHighlight::default();
        };
        LineHighlight {
            block,
            inline: &s.inline[s.inline_start[line]..s.inline_start[line + 1]],
        }
    }

    /// Parser hele teksten på hver side, så også slettede linjer får riktig syntaks.
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
    fn new(text: &str, blocks: &[Block], side: Side) -> Self {
        let line_offsets = LineOffsets::new(text);
        let count = line_offsets.line_count();
        let mut line_block = vec![None; count];
        let mut per_line: Vec<Vec<InlineHighlight>> = vec![Vec::new(); count];

        for (i, block) in blocks.iter().enumerate() {
            for line in block.lines(side) {
                line_block[line] = Some(i);
            }
            let base = block.offsets[idx(side)].start;
            for fragment in block.inner.iter().flatten() {
                let range = side.select(&fragment.offsets1, &fragment.offsets2);
                let diff_type = DiffType::from_sides(
                    !fragment.offsets1.is_empty(),
                    !fragment.offsets2.is_empty(),
                );
                split_into_lines(
                    &line_offsets,
                    base + range.start..base + range.end,
                    diff_type,
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
            line_block,
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
            // Spenn over flere linjer (blokkommentarer) blir liggende til siste linje
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

/// EXACT_RANGE-highlighter delt på linjer. Etter linjeslutt males bakgrunnen til
/// highlighteren som dekker linjeskiftet, men ikke en som starter på det
/// (`IterationState.getPastLineEndBackgroundAttributes` og `atBreak`-sjekken i `setAttributes`).
fn split_into_lines(
    lo: &LineOffsets,
    range: Range<usize>,
    diff_type: DiffType,
    out: &mut [Vec<InlineHighlight>],
) {
    if range.is_empty() {
        // `offsetToLogicalPosition`: offset rett etter '\n' hører til neste linje
        let line = line_of_offset(lo, range.start);
        let col = range.start - lo.line_start(line);
        out[line].push(InlineHighlight {
            cols: col..col,
            diff_type,
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
            diff_type,
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
    fn type_fra_tomme_sider() {
        let d = diff("a\nb\n", "a\nx\nb\n");
        assert_eq!(d.blocks()[0].diff_type, DiffType::Inserted);
        let d = diff("a\nx\nb\n", "a\nb\n");
        assert_eq!(d.blocks()[0].diff_type, DiffType::Deleted);
        let d = diff("a\nb\n", "a\nc\n");
        assert_eq!(d.blocks()[0].diff_type, DiffType::Modified);
    }

    #[test]
    fn blokk_ved_linje() {
        let d = diff("a\nb\nc\n", "a\nx\nc\n");
        assert_eq!(d.block_at(Side::Right, 1), Some(0));
        assert_eq!(d.block_at(Side::Right, 0), None);
        // Slettet linje: tom på høyre side, treffes på linja etter
        let d = diff("a\nx\nb\n", "a\nb\n");
        assert_eq!(d.block_at(Side::Left, 1), Some(0));
        assert_eq!(d.block_at(Side::Right, 1), Some(0));
        assert_eq!(d.block_at(Side::Right, 0), None);
    }

    #[test]
    fn linjer_per_side() {
        let d = diff("a\nb", "a\nb\n");
        assert_eq!(d.line_count(Side::Left), 2);
        assert_eq!(d.line_count(Side::Right), 3);
        assert_eq!(d.line(Side::Left, 1), "b");
        assert_eq!(d.line(Side::Right, 2), "");
    }

    #[test]
    fn linje_utenfor_blokk_har_ingen_highlight() {
        let d = diff("a\nb\n", "a\nc\n");
        assert_eq!(d.line_highlight(Side::Left, 0), LineHighlight::default());
        assert_eq!(d.line_highlight(Side::Left, 99), LineHighlight::default());
    }

    #[test]
    fn syntaks_per_linje() {
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
    fn line_of_offset_ved_grenser() {
        let lo = LineOffsets::new("ab\n\ncd");
        assert_eq!(line_of_offset(&lo, 0), 0);
        assert_eq!(line_of_offset(&lo, 2), 0);
        assert_eq!(line_of_offset(&lo, 3), 1);
        assert_eq!(line_of_offset(&lo, 4), 2);
        assert_eq!(line_of_offset(&lo, 6), 2);
    }

    #[test]
    fn fragment_over_linjeskift_deles() {
        let lo = LineOffsets::new("ab\ncd\nef");
        let mut out = vec![Vec::new(); 3];
        split_into_lines(&lo, 1..7, DiffType::Modified, &mut out);
        let h = |cols: Range<usize>, past_line_end| InlineHighlight {
            cols,
            diff_type: DiffType::Modified,
            past_line_end,
        };
        assert_eq!(out[0], vec![h(1..2, true)]);
        assert_eq!(out[1], vec![h(0..2, true)]);
        assert_eq!(out[2], vec![h(0..1, false)]);
    }

    #[test]
    fn fragment_som_starter_pa_linjeskiftet_forlenges_ikke() {
        let lo = LineOffsets::new("ab\ncd");
        let mut out = vec![Vec::new(); 2];
        split_into_lines(&lo, 2..4, DiffType::Inserted, &mut out);
        assert!(out[0].is_empty());
        assert_eq!(out[1][0].cols, 0..1);
    }

    #[test]
    fn tomt_fragment_etter_linjeskift_havner_pa_neste_linje() {
        let lo = LineOffsets::new("ab\ncd");
        let mut out = vec![Vec::new(); 2];
        split_into_lines(&lo, 3..3, DiffType::Deleted, &mut out);
        assert!(out[0].is_empty());
        assert_eq!(out[1][0].cols, 0..0);
    }
}
