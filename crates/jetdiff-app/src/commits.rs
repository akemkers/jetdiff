//! The commit list. Row 0 and `None` are Local changes. A hash instead of a row, so the selection survives a reload.

use std::rc::Rc;

use jetdiff_git::Commit;

use crate::step;

pub const LOCAL_CHANGES: &str = "Local changes";

#[derive(Default)]
pub struct Commits {
    commits: Rc<[Commit]>,
    selected: Option<String>,
    cursor: Option<String>,
}

impl Commits {
    pub fn new(commits: Vec<Commit>) -> Self {
        Self {
            commits: commits.into(),
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.commits.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn commit(&self, row: usize) -> Option<&Commit> {
        row.checked_sub(1).map(|c| &self.commits[c])
    }

    pub fn is_cursor(&self, row: usize) -> bool {
        self.hash_at(row) == self.cursor
    }

    pub fn is_selected(&self, row: usize) -> bool {
        self.hash_at(row) == self.selected
    }

    pub fn set_commits(&mut self, commits: Vec<Commit>) {
        self.commits = commits.into();
        // A commit that is gone, as after a rebase, gives Local changes
        if self.row_of(&self.cursor) == 0 {
            self.cursor = None;
        }
    }

    /// For when the view could not switch.
    pub fn set_selected(&mut self, hash: Option<String>) {
        self.selected = hash;
    }

    pub fn move_cursor(&mut self, down: bool) -> usize {
        let row = step(self.row_of(&self.cursor), down, self.len());
        self.cursor = self.hash_at(row);
        row
    }

    /// `Some` when the selection changed.
    pub fn select(&mut self, row: usize) -> Option<Option<String>> {
        let hash = self.hash_at(row);
        self.cursor = hash.clone();
        if hash == self.selected {
            return None;
        }
        self.selected = hash.clone();
        Some(hash)
    }

    pub fn select_at_cursor(&mut self) -> Option<Option<String>> {
        self.select(self.row_of(&self.cursor))
    }

    /// A commit that is not in the list gives row 0.
    fn row_of(&self, hash: &Option<String>) -> usize {
        hash.as_ref().map_or(0, |h| {
            self.commits
                .iter()
                .position(|c| &c.hash == h)
                .map_or(0, |c| c + 1)
        })
    }

    fn hash_at(&self, row: usize) -> Option<String> {
        self.commit(row).map(|c| c.hash.clone())
    }
}

pub fn age(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let ago =
        |n: i64, one: &str, many: &str| format!("{n} {} ago", if n == 1 { one } else { many });
    match seconds {
        s if s < MINUTE => "now".into(),
        s if s < HOUR => format!("{} min ago", s / MINUTE),
        s if s < DAY => ago(s / HOUR, "hour", "hours"),
        s if s < 30 * DAY => ago(s / DAY, "day", "days"),
        s if s < 365 * DAY => ago(s / (30 * DAY), "month", "months"),
        s => ago(s / (365 * DAY), "year", "years"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str) -> Commit {
        Commit {
            hash: hash.into(),
            subject: String::new(),
            time: 0,
        }
    }

    #[test]
    fn age_text() {
        assert_eq!(age(-5), "now");
        assert_eq!(age(59), "now");
        assert_eq!(age(120), "2 min ago");
        assert_eq!(age(3600), "1 hour ago");
        assert_eq!(age(5 * 3600), "5 hours ago");
        assert_eq!(age(86400), "1 day ago");
        assert_eq!(age(40 * 86400), "1 month ago");
        assert_eq!(age(800 * 86400), "2 years ago");
    }

    #[test]
    fn selects_only_on_change() {
        let mut c = Commits::new(vec![commit("a"), commit("b")]);
        assert_eq!(c.len(), 3);
        assert_eq!(c.select(0), None);
        assert_eq!(c.move_cursor(true), 1);
        assert_eq!(c.select_at_cursor(), Some(Some("a".into())));
        assert_eq!(c.select_at_cursor(), None);
        assert!(c.is_selected(1) && c.is_cursor(1));
    }

    #[test]
    fn missing_commit_gives_local_changes() {
        let mut c = Commits::new(vec![commit("a")]);
        c.move_cursor(true);
        c.set_commits(vec![commit("b")]);
        assert!(c.is_cursor(0));
    }
}
