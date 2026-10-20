//! Commitene under filtreet. Rad 0 er working tree, resten er commitene i rekkefølge.
//! Rader refereres med hash, så markøren og valget overlever at lista hentes på nytt.

use std::rc::Rc;

use jetdiff_git::Commit;

use crate::step;

/// Første rad, som er working tree.
pub const LOCAL_CHANGES: &str = "Lokale endringer";

#[derive(Default)]
pub struct Commits {
    commits: Rc<[Commit]>,
    /// Det som vises. `None` er working tree.
    selected: Option<String>,
    /// Raden j og k flytter, som hash. Enter åpner den.
    cursor: Option<String>,
}

impl Commits {
    pub fn new(commits: Vec<Commit>) -> Self {
        Self {
            commits: commits.into(),
            ..Self::default()
        }
    }

    /// Antall rader, med working tree.
    pub fn len(&self) -> usize {
        self.commits.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Commiten på en rad. `None` er working tree.
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
        // En commit som er borte, som etter en rebase, gir working tree
        if self.row_of(&self.cursor) == 0 {
            self.cursor = None;
        }
    }

    /// For når visningen ikke fikk byttet.
    pub fn set_selected(&mut self, hash: Option<String>) {
        self.selected = hash;
    }

    /// Gir raden markøren står på etterpå.
    pub fn move_cursor(&mut self, down: bool) -> usize {
        let row = step(self.row_of(&self.cursor), down, self.len());
        self.cursor = self.hash_at(row);
        row
    }

    /// Velger raden. Gir commiten som skal vises når valget endret seg.
    pub fn select(&mut self, row: usize) -> Option<Option<String>> {
        let hash = self.hash_at(row);
        self.cursor = hash.clone();
        if hash == self.selected {
            return None;
        }
        self.selected = hash.clone();
        Some(hash)
    }

    pub fn open_cursor(&mut self) -> Option<Option<String>> {
        self.select(self.row_of(&self.cursor))
    }

    /// En commit som ikke er i lista gir rad 0.
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

/// Sekunder siden, som «3 timer siden».
pub fn age(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let ago =
        |n: i64, one: &str, many: &str| format!("{n} {} siden", if n == 1 { one } else { many });
    match seconds {
        s if s < MINUTE => "nå".into(),
        s if s < HOUR => format!("{} min siden", s / MINUTE),
        s if s < DAY => ago(s / HOUR, "time", "timer"),
        s if s < 30 * DAY => ago(s / DAY, "dag", "dager"),
        s if s < 365 * DAY => ago(s / (30 * DAY), "måned", "måneder"),
        s => ago(s / (365 * DAY), "år", "år"),
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
    fn alder() {
        assert_eq!(age(-5), "nå");
        assert_eq!(age(59), "nå");
        assert_eq!(age(120), "2 min siden");
        assert_eq!(age(3600), "1 time siden");
        assert_eq!(age(5 * 3600), "5 timer siden");
        assert_eq!(age(86400), "1 dag siden");
        assert_eq!(age(40 * 86400), "1 måned siden");
        assert_eq!(age(800 * 86400), "2 år siden");
    }

    #[test]
    fn velger_bare_ved_endring() {
        let mut c = Commits::new(vec![commit("a"), commit("b")]);
        assert_eq!(c.len(), 3);
        assert_eq!(c.select(0), None);
        assert_eq!(c.move_cursor(true), 1);
        assert_eq!(c.open_cursor(), Some(Some("a".into())));
        assert_eq!(c.open_cursor(), None);
        assert!(c.is_selected(1) && c.is_cursor(1));
    }

    #[test]
    fn borte_commit_gir_working_tree() {
        let mut c = Commits::new(vec![commit("a")]);
        c.move_cursor(true);
        c.set_commits(vec![commit("b")]);
        assert!(c.is_cursor(0));
    }
}
