//! Port av `SmartLineChangeCorrector` fra `ChangeCorrector.kt`. To-stegs diff: gitt en
//! matching mellom "store" linjer, bygg matchingen for alle linjer ved å diffe gapene.

use crate::by_line::Line;
use crate::iterable::ChangeBuilder;
use crate::lcs::{self, Changes, DiffTooBig};
use crate::trim::expand;

pub(crate) struct SmartLineChangeCorrector<'a, 't> {
    /// Indeksen i `lines` for hver linje i delsekvensen.
    pub(crate) indexes1: &'a [usize],
    pub(crate) indexes2: &'a [usize],
    pub(crate) lines1: &'a [Line<'t>],
    pub(crate) lines2: &'a [Line<'t>],
}

impl SmartLineChangeCorrector<'_, '_> {
    pub(crate) fn build(&self, changes: &Changes) -> Result<Changes, DiffTooBig> {
        let (len1, len2) = (self.lines1.len(), self.lines2.len());
        let mut builder = ChangeBuilder::new(len1, len2);
        let (mut last1, mut last2) = (0, 0);

        for ch in changes.unchanged() {
            for i in 0..ch.end1 - ch.start1 {
                let index1 = self.indexes1[ch.start1 + i];
                let index2 = self.indexes2[ch.start2 + i];

                self.match_gap(&mut builder, last1, index1, last2, index2)?;
                builder.mark_equal(index1, index2);

                last1 = index1 + 1;
                last2 = index2 + 1;
            }
        }
        self.match_gap(&mut builder, last1, len1, last2, len2)?;
        Ok(builder.finish())
    }

    /// Matcher linjene i `[start1, end1)` mot `[start2, end2)`.
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
}
