//! Port of `MyersLCS.kt`. Linear memory with middle snake. Integers are `isize` as in the original.

use super::DiffTooBig;
use super::bitset::BitSet;

const DELTA_THRESHOLD_SIZE: isize = 20000;

pub(crate) struct MyersLcs<'a> {
    first: &'a [u32],
    second: &'a [u32],
    start1: isize,
    count1: isize,
    start2: isize,
    count2: isize,
    changes1: &'a mut BitSet,
    changes2: &'a mut BitSet,
    v_forward: Vec<isize>,
    v_backward: Vec<isize>,
}

impl<'a> MyersLcs<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        first: &'a [u32],
        second: &'a [u32],
        start1: usize,
        count1: usize,
        start2: usize,
        count2: usize,
        changes1: &'a mut BitSet,
        changes2: &'a mut BitSet,
    ) -> Self {
        changes1.set_range(start1, start1 + count1, true);
        changes2.set_range(start2, start2 + count2, true);
        let total = count1 + count2;
        Self {
            first,
            second,
            start1: start1 as isize,
            count1: count1 as isize,
            start2: start2 as isize,
            count2: count2 as isize,
            changes1,
            changes2,
            v_forward: vec![0; total + 1],
            v_backward: vec![0; total + 1],
        }
    }

    fn default_threshold(&self) -> isize {
        20000 + 10 * ((self.count1 + self.count2) as f64).sqrt() as isize
    }

    /// D bounded by `20000 + 10 * sqrt(N)`. Never fails; the rest is just marked as changed.
    pub(crate) fn execute_linear(&mut self) {
        let threshold = self.default_threshold();
        let _ = self.run(threshold, false);
    }

    pub(crate) fn execute_with_threshold(&mut self) -> Result<(), DiffTooBig> {
        let threshold = self.default_threshold().max(DELTA_THRESHOLD_SIZE);
        self.run(threshold, true)
    }

    fn run(&mut self, threshold: isize, throw: bool) -> Result<(), DiffTooBig> {
        if self.count1 == 0 || self.count2 == 0 {
            return Ok(());
        }
        let estimate = threshold.min(self.count1 + self.count2);
        self.execute(0, self.count1, 0, self.count2, estimate, throw)
    }

    // LCS( old[old_start, old_end), new[new_start, new_end) )
    fn execute(
        &mut self,
        old_start: isize,
        old_end: isize,
        new_start: isize,
        new_end: isize,
        difference_estimate: isize,
        throw: bool,
    ) -> Result<(), DiffTooBig> {
        debug_assert!(old_start <= old_end && new_start <= new_end);
        if !(old_start < old_end && new_start < new_end) {
            return Ok(());
        }

        let old_len = old_end - old_start;
        let new_len = new_end - new_start;
        self.v_forward[(new_len + 1) as usize] = 0;
        self.v_backward[(new_len + 1) as usize] = 0;
        let half_d = (difference_estimate + 1) / 2;
        let (mut xx, mut kk, mut td) = (-1isize, -1isize, -1isize);

        'outer: for d in 0..=half_d {
            let l = new_len + (-d).max(-new_len + ((d ^ new_len) & 1));
            let r = new_len + d.min(old_len - ((d ^ old_len) & 1));

            let mut k = l;
            while k <= r {
                let vf = &self.v_forward;
                let mut x = if k == l || k != r && vf[(k - 1) as usize] < vf[(k + 1) as usize] {
                    vf[(k + 1) as usize]
                } else {
                    vf[(k - 1) as usize] + 1
                };
                let y = x - k + new_len;
                x += self.common_forward(
                    old_start + x,
                    new_start + y,
                    (old_end - old_start - x).min(new_end - new_start - y),
                );
                self.v_forward[k as usize] = x;
                k += 2;
            }

            if (old_len - new_len) % 2 != 0 {
                let mut k = l;
                while k <= r {
                    if old_len - (d - 1) <= k
                        && k <= old_len + (d - 1)
                        && self.v_forward[k as usize]
                            + self.v_backward[(new_len + old_len - k) as usize]
                            >= old_len
                    {
                        xx = self.v_forward[k as usize];
                        kk = k;
                        td = 2 * d - 1;
                        break 'outer;
                    }
                    k += 2;
                }
            }

            let mut k = l;
            while k <= r {
                let vb = &self.v_backward;
                let mut x = if k == l || k != r && vb[(k - 1) as usize] < vb[(k + 1) as usize] {
                    vb[(k + 1) as usize]
                } else {
                    vb[(k - 1) as usize] + 1
                };
                let y = x - k + new_len;
                x += self.common_backward(
                    old_end - 1 - x,
                    new_end - 1 - y,
                    (old_end - old_start - x).min(new_end - new_start - y),
                );
                self.v_backward[k as usize] = x;
                k += 2;
            }

            if (old_len - new_len) % 2 == 0 {
                let mut k = l;
                while k <= r {
                    if old_len - d <= k
                        && k <= old_len + d
                        && self.v_forward[(old_len + new_len - k) as usize]
                            + self.v_backward[k as usize]
                            >= old_len
                    {
                        xx = old_len - self.v_backward[k as usize];
                        kk = old_len + new_len - k;
                        td = 2 * d;
                        break 'outer;
                    }
                    k += 2;
                }
            }
        }

        if td > 1 {
            let yy = xx - kk + new_len;
            let old_diff = (td + 1) / 2;
            if 0 < xx && 0 < yy {
                self.execute(
                    old_start,
                    old_start + xx,
                    new_start,
                    new_start + yy,
                    old_diff,
                    throw,
                )?;
            }
            if old_start + xx < old_end && new_start + yy < new_end {
                self.execute(
                    old_start + xx,
                    old_end,
                    new_start + yy,
                    new_end,
                    td - old_diff,
                    throw,
                )?;
            }
        } else if td >= 0 {
            let (mut x, mut y) = (old_start, new_start);
            while x < old_end && y < new_end {
                let common = self.common_forward(x, y, (old_end - x).min(new_end - y));
                if common > 0 {
                    self.add_unchanged(x, y, common);
                    x += common;
                    y += common;
                } else if old_end - old_start > new_end - new_start {
                    x += 1;
                } else {
                    y += 1;
                }
            }
        } else if throw {
            return Err(DiffTooBig);
        }
        Ok(())
    }

    fn add_unchanged(&mut self, start1: isize, start2: isize, count: isize) {
        let s1 = (self.start1 + start1) as usize;
        let s2 = (self.start2 + start2) as usize;
        self.changes1.set_range(s1, s1 + count as usize, false);
        self.changes2.set_range(s2, s2 + count as usize, false);
    }

    fn common_forward(&self, old_index: isize, new_index: isize, max_len: isize) -> isize {
        let max_len = max_len.min((self.count1 - old_index).min(self.count2 - new_index));
        let (mut x, mut y) = (old_index, new_index);
        while x - old_index < max_len
            && self.first[(self.start1 + x) as usize] == self.second[(self.start2 + y) as usize]
        {
            x += 1;
            y += 1;
        }
        x - old_index
    }

    fn common_backward(&self, old_index: isize, new_index: isize, max_len: isize) -> isize {
        let max_len = max_len.min(old_index.min(new_index) + 1);
        let (mut x, mut y) = (old_index, new_index);
        while old_index - x < max_len
            && self.first[(self.start1 + x) as usize] == self.second[(self.start2 + y) as usize]
        {
            x -= 1;
            y -= 1;
        }
        old_index - x
    }
}
