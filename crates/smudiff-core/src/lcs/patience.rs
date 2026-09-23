//! Port av `PatienceIntLCS.kt`. Brukes bare når Myers gir opp.

use super::DiffTooBig;
use super::bitset::BitSet;
use super::myers::MyersLcs;
use super::unique::unique_lcs;

pub(crate) struct PatienceLcs<'a> {
    first: &'a [u32],
    second: &'a [u32],
    count1: usize,
    count2: usize,
    changes1: &'a mut BitSet,
    changes2: &'a mut BitSet,
}

impl<'a> PatienceLcs<'a> {
    pub(crate) fn new(
        first: &'a [u32],
        second: &'a [u32],
        changes1: &'a mut BitSet,
        changes2: &'a mut BitSet,
    ) -> Self {
        Self {
            first,
            second,
            count1: first.len(),
            count2: second.len(),
            changes1,
            changes2,
        }
    }

    pub(crate) fn execute(&mut self, fail_on_small_reduction: bool) -> Result<(), DiffTooBig> {
        let counter = if fail_on_small_reduction { 2 } else { -1 };
        self.run(0, self.count1, 0, self.count2, counter)
    }

    fn run(
        &mut self,
        mut start1: usize,
        mut count1: usize,
        mut start2: usize,
        mut count2: usize,
        mut counter: i32,
    ) -> Result<(), DiffTooBig> {
        if count1 == 0 && count2 == 0 {
            return Ok(());
        }
        if count1 == 0 || count2 == 0 {
            self.add_change(start1, count1, start2, count2);
            return Ok(());
        }

        let start_offset = self.match_forward(start1, count1, start2, count2);
        start1 += start_offset;
        start2 += start_offset;
        count1 -= start_offset;
        count2 -= start_offset;

        let end_offset = self.match_backward(start1, count1, start2, count2);
        count1 -= end_offset;
        count2 -= end_offset;

        if count1 == 0 || count2 == 0 {
            self.add_change(start1, count1, start2, count2);
            return Ok(());
        }

        if counter == 0 {
            self.check_reduction(count1, count2)?;
        }
        counter = (counter - 1).max(-1);

        let Some((m1, m2)) = unique_lcs(self.first, self.second, start1, count1, start2, count2)
        else {
            if counter >= 0 {
                self.check_reduction(count1, count2)?;
            }
            MyersLcs::new(
                self.first,
                self.second,
                start1,
                count1,
                start2,
                count2,
                &mut *self.changes1,
                &mut *self.changes2,
            )
            .execute_linear();
            return Ok(());
        };

        let matched = m1.len();
        debug_assert!(matched > 0);

        self.run(start1, m1[0], start2, m2[0], counter)?;

        for i in 1..matched {
            let s1 = m1[i - 1] + 1;
            let s2 = m2[i - 1] + 1;
            let c1 = m1[i] - s1;
            let c2 = m2[i] - s2;
            if c1 > 0 || c2 > 0 {
                self.run(start1 + s1, c1, start2 + s2, c2, counter)?;
            }
        }

        let (s1, c1) = if m1[matched - 1] == count1 - 1 {
            (count1 - 1, 0)
        } else {
            (m1[matched - 1] + 1, count1 - m1[matched - 1] - 1)
        };
        let (s2, c2) = if m2[matched - 1] == count2 - 1 {
            (count2 - 1, 0)
        } else {
            (m2[matched - 1] + 1, count2 - m2[matched - 1] - 1)
        };
        self.run(start1 + s1, c1, start2 + s2, c2, counter)
    }

    fn match_forward(&self, start1: usize, count1: usize, start2: usize, count2: usize) -> usize {
        (0..count1.min(count2))
            .take_while(|&i| self.first[start1 + i] == self.second[start2 + i])
            .count()
    }

    fn match_backward(&self, start1: usize, count1: usize, start2: usize, count2: usize) -> usize {
        (1..=count1.min(count2))
            .take_while(|&i| self.first[start1 + count1 - i] == self.second[start2 + count2 - i])
            .count()
    }

    fn add_change(&mut self, start1: usize, count1: usize, start2: usize, count2: usize) {
        self.changes1.set_range(start1, start1 + count1, true);
        self.changes2.set_range(start2, start2 + count2, true);
    }

    fn check_reduction(&self, count1: usize, count2: usize) -> Result<(), DiffTooBig> {
        if count1 * 2 < self.count1 || count2 * 2 < self.count2 {
            return Ok(());
        }
        Err(DiffTooBig)
    }
}
