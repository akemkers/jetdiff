//! Port of `DiffIterableUtil.kt` and `iterables/*`. `Changes` plays the role of both
//! `DiffIterable` and `FairDiffIterable`. Being fair is not in the type; it follows from
//! how `Changes` is built, as in the original where `fair()` is just a wrapper.

use std::iter;

use crate::lcs::{Changes, Range};
use crate::trim::expand;

/// Builds `Changes` from `mark_equal` calls in ascending order. The holes become changes.
/// `adjust` can shrink each change before it is added; see `expand_change_builder`.
pub struct ChangeBuilder<F = fn(Range) -> Range> {
    index1: usize,
    index2: usize,
    len1: usize,
    len2: usize,
    ranges: Vec<Range>,
    adjust: F,
}

impl ChangeBuilder {
    pub fn new(len1: usize, len2: usize) -> Self {
        Self::with_adjust(len1, len2, |r| r)
    }
}

/// `ExpandChangeBuilder`: peels equal elements off the edges of each change.
pub fn expand_change_builder<'a, T: Eq>(
    data1: &'a [T],
    data2: &'a [T],
) -> ChangeBuilder<impl Fn(Range) -> Range + 'a> {
    ChangeBuilder::with_adjust(data1.len(), data2.len(), move |r: Range| {
        expand(data1, data2, r.start1, r.start2, r.end1, r.end2)
    })
}

impl<F: Fn(Range) -> Range> ChangeBuilder<F> {
    fn with_adjust(len1: usize, len2: usize, adjust: F) -> Self {
        Self {
            index1: 0,
            index2: 0,
            len1,
            len2,
            ranges: Vec::new(),
            adjust,
        }
    }

    pub fn index1(&self) -> usize {
        self.index1
    }

    pub fn index2(&self) -> usize {
        self.index2
    }

    pub fn mark_equal(&mut self, index1: usize, index2: usize) {
        self.mark_equal_count(index1, index2, 1);
    }

    pub fn mark_equal_count(&mut self, index1: usize, index2: usize, count: usize) {
        self.mark_equal_range(index1, index2, index1 + count, index2 + count);
    }

    /// The lengths can differ; then the result is not fair (used at character level).
    pub fn mark_equal_range(&mut self, index1: usize, index2: usize, end1: usize, end2: usize) {
        if index1 == end1 && index2 == end2 {
            return;
        }

        assert!(self.index1 <= index1 && self.index2 <= index2);
        assert!(index1 <= end1 && index2 <= end2);

        if self.index1 != index1 || self.index2 != index2 {
            self.add_change(self.index1, self.index2, index1, index2);
        }
        self.index1 = end1;
        self.index2 = end2;
    }

    fn add_change(&mut self, start1: usize, start2: usize, end1: usize, end2: usize) {
        let range = (self.adjust)(Range::new(start1, end1, start2, end2));
        if !range.is_empty() {
            self.ranges.push(range);
        }
    }

    pub fn finish(mut self) -> Changes {
        assert!(self.index1 <= self.len1 && self.index2 <= self.len2);

        if self.len1 != self.index1 || self.len2 != self.index2 {
            self.add_change(self.index1, self.index2, self.len1, self.len2);
            self.index1 = self.len1;
            self.index2 = self.len2;
        }
        Changes {
            len1: self.len1,
            len2: self.len2,
            ranges: self.ranges,
        }
    }
}

impl Changes {
    /// `DiffIterableUtil.createUnchanged`: the changes are the holes between the given unchanged ranges.
    pub fn from_unchanged(unchanged: &[Range], len1: usize, len2: usize) -> Self {
        let mut ranges = Vec::new();
        let mut last = (0, 0);
        for u in unchanged
            .iter()
            .chain(iter::once(&Range::new(len1, len1, len2, len2)))
        {
            let gap = Range::new(last.0, u.start1, last.1, u.start2);
            if !gap.is_empty() {
                ranges.push(gap);
            }
            last = (u.end1, u.end2);
        }
        Self { len1, len2, ranges }
    }

    /// `SubiterableDiffIterable`: the changes clipped to a window, relative to the window's start.
    /// `first_index` is the first change that can hit the window, so we don't have to search from the start every time.
    pub fn subiterable(
        changed: &[Range],
        start1: usize,
        end1: usize,
        start2: usize,
        end2: usize,
        first_index: usize,
    ) -> Self {
        let mut ranges = Vec::new();
        for range in &changed[first_index..] {
            if range.end1 < start1 || range.end2 < start2 {
                continue;
            }
            if range.start1 > end1 || range.start2 > end2 {
                break;
            }
            let r = Range::new(
                start1.max(range.start1) - start1,
                end1.min(range.end1) - start1,
                start2.max(range.start2) - start2,
                end2.min(range.end2) - start2,
            );
            if !r.is_empty() {
                ranges.push(r);
            }
        }
        Self {
            len1: end1 - start1,
            len2: end2 - start2,
            ranges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuples(changes: &Changes) -> Vec<(usize, usize, usize, usize)> {
        changes
            .ranges
            .iter()
            .map(|r| (r.start1, r.end1, r.start2, r.end2))
            .collect()
    }

    #[test]
    fn builder_makes_holes_into_changes() {
        let mut b = ChangeBuilder::new(5, 6);
        b.mark_equal(0, 0);
        b.mark_equal_count(2, 3, 2);
        let changes = b.finish();
        assert_eq!(tuples(&changes), vec![(1, 2, 1, 3), (4, 5, 5, 6)]);
    }

    #[test]
    fn expand_builder_trims_the_edges() {
        let a = [1, 2, 3, 4];
        let b = [1, 9, 3, 4];
        let mut builder = expand_change_builder(&a, &b);
        builder.mark_equal(3, 3);
        let changes = builder.finish();
        assert_eq!(tuples(&changes), vec![(1, 2, 1, 2)]);

        let mut builder = expand_change_builder(&a, &a);
        builder.mark_equal(3, 3);
        assert!(builder.finish().ranges.is_empty());
    }

    #[test]
    fn from_unchanged_gives_the_holes() {
        let unchanged = [Range::new(0, 2, 0, 2), Range::new(3, 4, 5, 6)];
        let changes = Changes::from_unchanged(&unchanged, 6, 6);
        assert_eq!(tuples(&changes), vec![(2, 3, 2, 5), (4, 6, 6, 6)]);
        assert_eq!(changes.unchanged().collect::<Vec<_>>(), unchanged);
    }

    #[test]
    fn subiterable_clips() {
        let changed = [Range::new(1, 3, 1, 2), Range::new(5, 6, 4, 6)];
        let sub = Changes::subiterable(&changed, 2, 6, 2, 5, 0);
        assert_eq!((sub.len1, sub.len2), (4, 3));
        assert_eq!(tuples(&sub), vec![(0, 1, 0, 0), (3, 4, 2, 3)]);
    }
}
