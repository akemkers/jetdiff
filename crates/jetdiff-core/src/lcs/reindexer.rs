//! Port of `Reindexer.kt`. Removes elements that don't exist on the other side before the LCS,
//! and maps the result back to the original indices afterwards.

use super::ChangeBuilder;
use super::bitset::BitSet;

#[derive(Default)]
pub(crate) struct Reindexer {
    old_indices: [Vec<usize>; 2],
    original_len: [usize; 2],
    discarded_len: [usize; 2],
}

impl Reindexer {
    pub(crate) fn discard_unique(&mut self, ints1: &[u32], ints2: &[u32]) -> (Vec<u32>, Vec<u32>) {
        let d1 = self.discard(ints2, ints1, 0);
        let d2 = self.discard(&d1, ints2, 1);
        (d1, d2)
    }

    fn discard(&mut self, needed: &[u32], to_discard: &[u32], side: usize) -> Vec<u32> {
        self.original_len[side] = to_discard.len();
        let mut sorted = needed.to_vec();
        sorted.sort_unstable();
        let mut discarded = Vec::with_capacity(to_discard.len());
        let mut old = Vec::with_capacity(to_discard.len());
        for (i, &x) in to_discard.iter().enumerate() {
            if sorted.binary_search(&x).is_ok() {
                discarded.push(x);
                old.push(i);
            }
        }
        self.discarded_len[side] = discarded.len();
        self.old_indices[side] = old;
        discarded
    }

    pub(crate) fn reindex(&self, discarded: &[BitSet; 2], builder: &mut ChangeBuilder) {
        let [len1, len2] = self.original_len;
        let [dlen1, dlen2] = self.discarded_len;
        let [old1, old2] = &self.old_indices;

        let (changes1, changes2) = if dlen1 == len1 && dlen2 == len2 {
            (discarded[0].clone(), discarded[1].clone())
        } else {
            let mut c1 = BitSet::new(len1);
            let mut c2 = BitSet::new(len2);
            let (mut x, mut y) = (0, 0);
            while x < dlen1 || y < dlen2 {
                if x < dlen1 && y < dlen2 && !discarded[0].get(x) && !discarded[1].get(y) {
                    x = increment(old1, x, &mut c1, len1);
                    y = increment(old2, y, &mut c2, len2);
                } else if discarded[0].get(x) {
                    c1.set(old1[x], true);
                    x = increment(old1, x, &mut c1, len1);
                } else if discarded[1].get(y) {
                    c2.set(old2[y], true);
                    y = increment(old2, y, &mut c2, len2);
                }
            }
            if dlen1 == 0 {
                c1.set_range(0, len1, true);
            } else {
                c1.set_range(0, old1[0], true);
            }
            if dlen2 == 0 {
                c2.set_range(0, len2, true);
            } else {
                c2.set_range(0, old2[0], true);
            }
            (c1, c2)
        };

        let (mut x, mut y) = (0, 0);
        while x < len1 && y < len2 {
            let start_x = x;
            while x < len1 && y < len2 && !changes1.get(x) && !changes2.get(y) {
                x += 1;
                y += 1;
            }
            if x > start_x {
                builder.add_equal(x - start_x);
            }
            let (mut dx, mut dy) = (0, 0);
            while x < len1 && changes1.get(x) {
                dx += 1;
                x += 1;
            }
            while y < len2 && changes2.get(y) {
                dy += 1;
                y += 1;
            }
            if dx != 0 || dy != 0 {
                builder.add_change(dx, dy);
            }
        }
        if x != len1 || y != len2 {
            builder.add_change(len1 - x, len2 - y);
        }
    }
}

fn increment(indices: &[usize], i: usize, set: &mut BitSet, len: usize) -> usize {
    let end = if i + 1 < indices.len() {
        indices[i + 1]
    } else {
        len
    };
    set.set_range(indices[i] + 1, end, true);
    i + 1
}
