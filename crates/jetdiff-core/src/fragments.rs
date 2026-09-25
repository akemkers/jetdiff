//! Resultattyper og linjeoffsets. Port av `LineOffsetsImpl.kt`, `LineFragment`/`DiffFragment`
//! og `convertIntoLineFragments` fra `ComparisonManagerImpl.java`. Offsets er UTF-8 bytes.

use std::ops::Range;

use crate::lcs::Changes;

/// Endring inni et `LineFragment`, offsets relativt til fragmentets start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFragment {
    pub offsets1: Range<usize>,
    pub offsets2: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFragment {
    /// Linjer, halvåpne og 0-baserte.
    pub lines1: Range<usize>,
    pub lines2: Range<usize>,
    /// Hele linjer inkludert `'\n'`, som kan mangle på siste linje.
    pub offsets1: Range<usize>,
    pub offsets2: Range<usize>,
    /// `None` er hel-linje-highlight. `Some(vec![])` er endret, men ingenting synlig under policyen.
    pub inner: Option<Vec<DiffFragment>>,
}

impl LineFragment {
    /// `LineFragmentImpl`: ett inner fragment som dekker alt blir `None`.
    pub fn new(
        lines1: Range<usize>,
        lines2: Range<usize>,
        offsets1: Range<usize>,
        offsets2: Range<usize>,
        inner: Option<Vec<DiffFragment>>,
    ) -> Self {
        assert!(lines1.start != lines1.end || lines2.start != lines2.end);
        assert!(lines1.start <= lines1.end && lines2.start <= lines2.end);
        assert!(offsets1.start <= offsets1.end && offsets2.start <= offsets2.end);
        let inner = drop_whole_changed_fragments(inner, offsets1.len(), offsets2.len());
        Self {
            lines1,
            lines2,
            offsets1,
            offsets2,
            inner,
        }
    }

    pub fn with_inner(&self, inner: Option<Vec<DiffFragment>>) -> Self {
        Self::new(
            self.lines1.clone(),
            self.lines2.clone(),
            self.offsets1.clone(),
            self.offsets2.clone(),
            inner,
        )
    }
}

fn drop_whole_changed_fragments(
    fragments: Option<Vec<DiffFragment>>,
    len1: usize,
    len2: usize,
) -> Option<Vec<DiffFragment>> {
    match fragments.as_deref() {
        Some([f]) if f.offsets1 == (0..len1) && f.offsets2 == (0..len2) => None,
        _ => fragments,
    }
}

/// Linjeslutter delt på `'\n'`. Teksten har alltid minst én linje, siste kan være tom.
#[derive(Debug, Clone)]
pub struct LineOffsets {
    line_ends: Vec<usize>,
}

impl LineOffsets {
    pub fn new(text: &str) -> Self {
        let mut line_ends: Vec<usize> = text
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| i)
            .collect();
        line_ends.push(text.len());
        Self { line_ends }
    }

    pub fn line_count(&self) -> usize {
        self.line_ends.len()
    }

    pub fn line_start(&self, line: usize) -> usize {
        assert!(line < self.line_count());
        if line == 0 {
            0
        } else {
            self.line_ends[line - 1] + 1
        }
    }

    /// Uten `'\n'`.
    pub fn line_end(&self, line: usize) -> usize {
        self.line_ends[line]
    }

    /// Med `'\n'`, som mangler på siste linje.
    pub fn line_end_with_newline(&self, line: usize) -> usize {
        let last = line == self.line_count() - 1;
        self.line_ends[line] + usize::from(!last)
    }

    /// Linjeinnhold uten `'\n'`. `ComparisonManagerImpl.getLineContents`.
    pub fn line_contents<'t>(&self, text: &'t str) -> Vec<&'t str> {
        (0..self.line_count())
            .map(|line| &text[self.line_start(line)..self.line_end(line)])
            .collect()
    }
}

pub fn convert_into_line_fragments(
    line_offsets1: &LineOffsets,
    line_offsets2: &LineOffsets,
    changes: &Changes,
) -> Vec<LineFragment> {
    changes
        .ranges
        .iter()
        .map(|r| {
            create_line_fragment(
                r.start1..r.end1,
                r.start2..r.end2,
                line_offsets1,
                line_offsets2,
            )
        })
        .collect()
}

pub fn create_line_fragment(
    lines1: Range<usize>,
    lines2: Range<usize>,
    line_offsets1: &LineOffsets,
    line_offsets2: &LineOffsets,
) -> LineFragment {
    LineFragment {
        offsets1: offsets(line_offsets1, lines1.start, lines1.end),
        offsets2: offsets(line_offsets2, lines2.start, lines2.end),
        lines1,
        lines2,
        inner: None,
    }
}

/// Tom side peker på starten av linja, eller slutten av teksten etter siste linje.
/// Da havner en '\n' som legges til på slutten utenfor fragmentet.
fn offsets(line_offsets: &LineOffsets, start: usize, end: usize) -> Range<usize> {
    if start == end {
        let offset = if start < line_offsets.line_count() {
            line_offsets.line_start(start)
        } else {
            line_offsets.line_end_with_newline(line_offsets.line_count() - 1)
        };
        offset..offset
    } else {
        line_offsets.line_start(start)..line_offsets.line_end_with_newline(end - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linjeoffsets() {
        let lo = LineOffsets::new("a\nbc\n");
        assert_eq!(lo.line_count(), 3);
        assert_eq!(lo.line_contents("a\nbc\n"), vec!["a", "bc", ""]);
        assert_eq!((lo.line_start(1), lo.line_end(1)), (2, 4));
        assert_eq!(lo.line_end_with_newline(1), 5);
        assert_eq!(lo.line_end_with_newline(2), 5);

        let lo = LineOffsets::new("");
        assert_eq!(lo.line_count(), 1);
        assert_eq!(lo.line_contents(""), vec![""]);
    }

    #[test]
    fn offsets_for_tom_side() {
        let lo = LineOffsets::new("a\nb");
        assert_eq!(offsets(&lo, 0, 0), 0..0);
        assert_eq!(offsets(&lo, 1, 1), 2..2);
        // Etter siste linje: slutten av teksten, uten ekstra '\n'
        assert_eq!(offsets(&lo, 2, 2), 3..3);

        let lo = LineOffsets::new("a\nb\n");
        assert_eq!(offsets(&lo, 2, 2), 4..4);
        assert_eq!(offsets(&lo, 3, 3), 4..4);
    }

    #[test]
    fn offsets_inkluderer_newline() {
        let lo = LineOffsets::new("a\nb\nc");
        assert_eq!(offsets(&lo, 0, 1), 0..2);
        assert_eq!(offsets(&lo, 1, 3), 2..5);
        assert_eq!(offsets(&lo, 2, 3), 4..5);
    }
}
