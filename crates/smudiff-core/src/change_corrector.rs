//! Port av `ChangeCorrector.kt`. To-stegs diff: gitt en matching mellom delsekvenser,
//! bygg matchingen for hele sekvensene ved å diffe gapene mellom matchede elementer.

use crate::by_line::Line;
use crate::iterable::ChangeBuilder;
use crate::lcs::{self, Changes, DiffTooBig};
use crate::trim::expand;

pub(crate) trait ChangeCorrector {
    fn lengths(&self) -> (usize, usize);

    /// Matcher elementene i `[start1, end1)` mot `[start2, end2)`.
    fn match_gap(
        &self,
        builder: &mut ChangeBuilder,
        start1: usize,
        end1: usize,
        start2: usize,
        end2: usize,
    ) -> Result<(), DiffTooBig>;

    /// Halvåpent område i hele sekvensen for element `index` i delsekvensen.
    fn original_range1(&self, index: usize) -> (usize, usize);

    fn original_range2(&self, index: usize) -> (usize, usize);

    fn build(&self, changes: &Changes) -> Result<Changes, DiffTooBig> {
        let (len1, len2) = self.lengths();
        let mut builder = ChangeBuilder::new(len1, len2);
        let (mut last1, mut last2) = (0, 0);

        for ch in changes.unchanged() {
            for i in 0..ch.end1 - ch.start1 {
                let (start1, end1) = self.original_range1(ch.start1 + i);
                let (start2, end2) = self.original_range2(ch.start2 + i);

                self.match_gap(&mut builder, last1, start1, last2, start2)?;
                builder.mark_equal_range(start1, start2, end1, end2);

                last1 = end1;
                last2 = end2;
            }
        }
        self.match_gap(&mut builder, last1, len1, last2, len2)?;
        Ok(builder.finish())
    }
}

/// Steg 2 i `compareSmart`: fyll gapene mellom matchede "store" linjer med full linjediff.
pub(crate) struct SmartLineChangeCorrector<'a, 't> {
    pub(crate) indexes1: &'a [usize],
    pub(crate) indexes2: &'a [usize],
    pub(crate) lines1: &'a [Line<'t>],
    pub(crate) lines2: &'a [Line<'t>],
}

impl ChangeCorrector for SmartLineChangeCorrector<'_, '_> {
    fn lengths(&self) -> (usize, usize) {
        (self.lines1.len(), self.lines2.len())
    }

    fn match_gap(
        &self,
        builder: &mut ChangeBuilder,
        start1: usize,
        end1: usize,
        start2: usize,
        end2: usize,
    ) -> Result<(), DiffTooBig> {
        let expand = expand(self.lines1, self.lines2, start1, start2, end1, end2);

        let inner1 = &self.lines1[expand.start1..expand.end1];
        let inner2 = &self.lines2[expand.start2..expand.end2];
        let inner_changes = lcs::diff(inner1, inner2)?;

        builder.mark_equal_range(start1, start2, expand.start1, expand.start2);

        for chunk in inner_changes.unchanged() {
            builder.mark_equal_count(
                expand.start1 + chunk.start1,
                expand.start2 + chunk.start2,
                chunk.end1 - chunk.start1,
            );
        }

        builder.mark_equal_range(expand.end1, expand.end2, end1, end2);
        Ok(())
    }

    fn original_range1(&self, index: usize) -> (usize, usize) {
        let offset = self.indexes1[index];
        (offset, offset + 1)
    }

    fn original_range2(&self, index: usize) -> (usize, usize) {
        let offset = self.indexes2[index];
        (offset, offset + 1)
    }
}
