//! Excerpts: the rows of some hunks with unchanged rows around them, and the lines they have on each
//! side when they are shown in Split.

use std::ops::Range;

use jetdiff_core::side::Side;

use super::UnifiedDiff;
use crate::model::idx;

impl UnifiedDiff {
    /// The rows of `hunks` with `context` unchanged rows around each, in order. Excerpts that
    /// overlap or touch become one.
    pub fn excerpts(&self, hunks: &[usize], context: usize) -> Vec<Range<usize>> {
        let mut rows: Vec<Range<usize>> = hunks
            .iter()
            .map(|&hunk| {
                let rows = &self.hunk_rows[hunk];
                rows.start.saturating_sub(context)..(rows.end + context).min(self.rows.len())
            })
            .collect();
        rows.sort_by_key(|rows| rows.start);
        let mut excerpts: Vec<Range<usize>> = Vec::with_capacity(rows.len());
        for rows in rows {
            match excerpts.last_mut() {
                Some(last) if rows.start <= last.end => last.end = last.end.max(rows.end),
                _ => excerpts.push(rows),
            }
        }
        excerpts
    }

    /// The lines `rows` has on `side`. Without any, it is the empty range where the first hunk is.
    pub fn side_lines(&self, rows: Range<usize>, side: Side) -> Range<usize> {
        let mut lines = self.rows[rows.clone()]
            .iter()
            .filter_map(|row| row.lines[idx(side)]);
        match lines.next() {
            Some(first) => first..lines.next_back().unwrap_or(first) + 1,
            None => {
                let hunks = self.hunks_in(rows);
                let start = self
                    .diff
                    .hunks()
                    .get(hunks.start)
                    .map_or(0, |hunk| hunk.lines(side).start);
                start..start
            }
        }
    }

    /// The hunks with rows in `rows`.
    pub fn hunks_in(&self, rows: Range<usize>) -> Range<usize> {
        let start = self
            .hunk_rows
            .partition_point(|hunk| hunk.end <= rows.start);
        let end = self.hunk_rows.partition_point(|hunk| hunk.start < rows.end);
        start..end.max(start)
    }
}
