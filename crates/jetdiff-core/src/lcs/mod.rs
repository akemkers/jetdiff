//! The LCS engine. Port of `com.intellij.util.diff.Diff` with Myers, Reindexer and the patience fallback.
//! Kept close to the original so the results match IntelliJ's.

mod bitset;
mod myers;
mod patience;
mod reindexer;
mod unique;

use std::collections::HashMap;
use std::hash::Hash;

use bitset::BitSet;
use myers::MyersLcs;
use patience::PatienceLcs;
use reindexer::Reindexer;

/// Myers gave up and patience could not reduce the problem enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffTooBig;

/// A changed range, half-open intervals on each side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub start1: usize,
    pub end1: usize,
    pub start2: usize,
    pub end2: usize,
}

impl Range {
    pub fn new(start1: usize, end1: usize, start2: usize, end2: usize) -> Self {
        Self {
            start1,
            end1,
            start2,
            end2,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start1 == self.end1 && self.start2 == self.end2
    }
}

/// The result of a diff: the changes in ascending order. Everything between them is equal 1:1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub len1: usize,
    pub len2: usize,
    pub ranges: Vec<Range>,
}

impl Changes {
    /// Unchanged ranges between the changes, including before the first and after the last.
    pub fn unchanged(&self) -> impl Iterator<Item = Range> + '_ {
        let mut last = (0, 0);
        self.ranges
            .iter()
            .copied()
            .chain(std::iter::once(Range::new(
                self.len1, self.len1, self.len2, self.len2,
            )))
            .filter_map(move |r| {
                let u = Range::new(last.0, r.start1, last.1, r.start2);
                last = (r.end1, r.end2);
                (!u.is_empty()).then_some(u)
            })
    }
}

/// Compares two sequences based on `Eq`/`Hash`. Corresponds to `DiffIterableUtil.diff`.
pub fn diff<'t, T: Hash + Eq>(a: &'t [T], b: &'t [T]) -> Result<Changes, DiffTooBig> {
    let start = common_prefix(a, b);
    let end = common_suffix(a, b, start);

    let mut builder = ChangeBuilder::new(start, a.len(), b.len());
    if fast_path(a.len(), b.len(), start, end, &mut builder) {
        return Ok(builder.finish());
    }

    let mut numbers: HashMap<&'t T, u32> =
        HashMap::with_capacity(a.len() + b.len() - 2 * (start + end));
    let mut enumerate = |xs: &'t [T]| -> Vec<u32> {
        xs.iter()
            .map(|x| {
                let next = numbers.len() as u32 + 1;
                *numbers.entry(x).or_insert(next)
            })
            .collect()
    };
    let ints1 = enumerate(&a[start..a.len() - end]);
    let ints2 = enumerate(&b[start..b.len() - end]);
    build(&ints1, &ints2, builder)
}

/// Compares two int sequences directly. Corresponds to `Diff.buildChanges(IntArray, IntArray)`.
pub fn diff_ints(a: &[u32], b: &[u32]) -> Result<Changes, DiffTooBig> {
    let start = common_prefix(a, b);
    let end = common_suffix(a, b, start);

    let mut builder = ChangeBuilder::new(start, a.len(), b.len());
    if fast_path(a.len(), b.len(), start, end, &mut builder) {
        return Ok(builder.finish());
    }
    build(&a[start..a.len() - end], &b[start..b.len() - end], builder)
}

fn fast_path(
    len1: usize,
    len2: usize,
    start: usize,
    end: usize,
    builder: &mut ChangeBuilder,
) -> bool {
    let t1 = len1 - start - end;
    let t2 = len2 - start - end;
    if t1 != 0 && t2 != 0 {
        return false;
    }
    if t1 != 0 || t2 != 0 {
        builder.add_change(t1, t2);
    }
    true
}

fn build(ints1: &[u32], ints2: &[u32], mut builder: ChangeBuilder) -> Result<Changes, DiffTooBig> {
    let mut reindexer = Reindexer::default();
    let (d1, d2) = reindexer.discard_unique(ints1, ints2);

    if d1.is_empty() && d2.is_empty() {
        builder.add_change(ints1.len(), ints2.len());
        return Ok(builder.finish());
    }

    let mut changes = [BitSet::new(d1.len()), BitSet::new(d2.len())];
    let [c1, c2] = &mut changes;
    let myers = MyersLcs::new(&d1, &d2, 0, d1.len(), 0, d2.len(), c1, c2).execute_with_threshold();
    if myers.is_err() {
        changes = [BitSet::new(d1.len()), BitSet::new(d2.len())];
        let [c1, c2] = &mut changes;
        PatienceLcs::new(&d1, &d2, c1, c2).execute(true)?;
    }

    reindexer.reindex(&changes, &mut builder);
    Ok(builder.finish())
}

fn common_prefix<T: Eq>(a: &[T], b: &[T]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

fn common_suffix<T: Eq>(a: &[T], b: &[T], start: usize) -> usize {
    let max = a.len().min(b.len()) - start;
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take(max)
        .take_while(|(x, y)| x == y)
        .count()
}

/// Builds the list of changes from a stream of `add_equal`/`add_change`. Corresponds to `Diff.ChangeBuilder`.
pub(crate) struct ChangeBuilder {
    index1: usize,
    index2: usize,
    len1: usize,
    len2: usize,
    ranges: Vec<Range>,
}

impl ChangeBuilder {
    fn new(start_shift: usize, len1: usize, len2: usize) -> Self {
        Self {
            index1: start_shift,
            index2: start_shift,
            len1,
            len2,
            ranges: Vec::new(),
        }
    }

    pub(crate) fn add_equal(&mut self, len: usize) {
        self.index1 += len;
        self.index2 += len;
    }

    pub(crate) fn add_change(&mut self, first: usize, second: usize) {
        self.ranges.push(Range::new(
            self.index1,
            self.index1 + first,
            self.index2,
            self.index2 + second,
        ));
        self.index1 += first;
        self.index2 += second;
    }

    fn finish(self) -> Changes {
        Changes {
            len1: self.len1,
            len2: self.len2,
            ranges: self.ranges,
        }
    }
}

#[cfg(test)]
mod tests;
