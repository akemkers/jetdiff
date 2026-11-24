//! The commit list: Branch at the top, with Local changes and the branch's own commits under it as its
//! slices. Without own commits Branch is the same as Local changes, so its row is hidden. On the base
//! branch the newest commits on HEAD follow Local changes instead. The cursor and the selection are kept as what
//! the row shows, not as a row, so they survive a read.

use jetdiff_git::{Commit, History};

use crate::step;

pub const LOCAL_CHANGES: &str = "Local changes";
/// Enough of a hash to tell commits apart, as git shows it.
const SHORT_HASH: usize = 7;
/// The Branch row when HEAD is detached.
const BRANCH: &str = "Branch";

/// What is selected in the commit list, and so what the repo source reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selected {
    Branch,
    LocalChanges,
    Commit(String),
}

impl Selected {
    /// Where the selection goes when its row is gone: to Branch, or to Local changes when the branch
    /// has no commits of its own and Branch is hidden.
    pub(crate) fn within(self, history: &History) -> Self {
        match self {
            Self::Commit(hash) if history.commits.iter().any(|commit| commit.hash == hash) => {
                Self::Commit(hash)
            }
            Self::Commit(_) | Self::Branch if history.own => Self::Branch,
            Self::Commit(_) | Self::Branch | Self::LocalChanges => Self::LocalChanges,
        }
    }
}

pub enum CommitRow<'a> {
    /// The branch name, or `BRANCH`.
    Branch(&'a str),
    LocalChanges,
    Commit(&'a Commit),
}

pub struct Commits {
    branch_name: Option<String>,
    history: History,
    selected: Selected,
    cursor: Selected,
}

impl Commits {
    /// Branch is selected, or Local changes when it is hidden.
    pub fn new(branch_name: Option<String>, history: History) -> Self {
        let selected = Selected::Branch.within(&history);
        Self {
            branch_name,
            history,
            cursor: selected.clone(),
            selected,
        }
    }

    pub fn len(&self) -> usize {
        self.first_commit_row() + self.history.commits.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn branch_label(&self) -> &str {
        self.branch_name.as_deref().unwrap_or(BRANCH)
    }

    pub fn row(&self, row: usize) -> CommitRow<'_> {
        match row.checked_sub(self.first_commit_row()) {
            Some(commit) => CommitRow::Commit(&self.history.commits[commit]),
            None if row == 0 && self.has_branch_row() => CommitRow::Branch(self.branch_label()),
            None => CommitRow::LocalChanges,
        }
    }

    /// In levels. The slices sit one level under Branch.
    pub fn indent(&self, row: usize) -> usize {
        usize::from(self.has_branch_row() && row > 0)
    }

    pub fn is_cursor(&self, row: usize) -> bool {
        self.row_of(&self.cursor) == row
    }

    pub fn is_selected(&self, row: usize) -> bool {
        self.row_of(&self.selected) == row
    }

    /// What is selected, in words, for the model that writes a walkthrough.
    pub fn describe_selected(&self) -> String {
        match &self.selected {
            Selected::Branch => format!(
                "Branch {}, committed and not, against where it branched off its base branch",
                self.branch_label()
            ),
            Selected::LocalChanges => {
                "Local changes, everything not committed, against HEAD".into()
            }
            Selected::Commit(hash) => {
                let commit = self
                    .history
                    .commits
                    .iter()
                    .find(|commit| &commit.hash == hash);
                let subject = commit.map_or("", |commit| commit.subject.as_str());
                format!("Commit {}: {subject}", &hash[..hash.len().min(SHORT_HASH)])
            }
        }
    }

    /// After a read. `selected` is what the repo source shows now.
    pub fn set_commits(
        &mut self,
        branch_name: Option<String>,
        history: History,
        selected: Selected,
    ) {
        self.branch_name = branch_name;
        self.history = history;
        self.cursor = self.cursor.clone().within(&self.history);
        self.selected = selected;
    }

    /// For when the view could not switch.
    pub fn set_selected(&mut self, selected: Selected) {
        self.selected = selected;
    }

    pub fn move_cursor(&mut self, down: bool) -> usize {
        let row = step(self.row_of(&self.cursor), down, self.len());
        self.cursor = self.selected_at(row);
        row
    }

    /// `Some` when the selection changed.
    pub fn select(&mut self, row: usize) -> Option<Selected> {
        let selected = self.selected_at(row);
        self.cursor = selected.clone();
        if selected == self.selected {
            return None;
        }
        self.selected = selected.clone();
        Some(selected)
    }

    pub fn select_at_cursor(&mut self) -> Option<Selected> {
        self.select(self.row_of(&self.cursor))
    }

    /// From the selection, not the cursor.
    pub fn row_next_to_selected(&self, down: bool) -> usize {
        step(self.row_of(&self.selected), down, self.len())
    }

    fn has_branch_row(&self) -> bool {
        self.history.own
    }

    /// Branch and Local changes come before the commits.
    fn first_commit_row(&self) -> usize {
        usize::from(self.has_branch_row()) + 1
    }

    /// A row that is gone gives where `Selected::within` sends it.
    fn row_of(&self, selected: &Selected) -> usize {
        let branch_row = usize::from(self.has_branch_row());
        match selected.clone().within(&self.history) {
            Selected::Branch => 0,
            Selected::LocalChanges => branch_row,
            Selected::Commit(hash) => {
                let commit = self
                    .history
                    .commits
                    .iter()
                    .position(|commit| commit.hash == hash);
                self.first_commit_row() + commit.unwrap_or_default()
            }
        }
    }

    fn selected_at(&self, row: usize) -> Selected {
        match self.row(row) {
            CommitRow::Branch(_) => Selected::Branch,
            CommitRow::LocalChanges => Selected::LocalChanges,
            CommitRow::Commit(commit) => Selected::Commit(commit.hash.clone()),
        }
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

    fn own(hashes: &[&str]) -> History {
        History {
            commits: hashes.iter().map(|hash| commit(hash)).collect(),
            own: true,
        }
    }

    fn earlier(hashes: &[&str]) -> History {
        History {
            own: false,
            ..own(hashes)
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

    fn subjects(commits: &Commits) -> Vec<(usize, String)> {
        (0..commits.len())
            .map(|row| {
                let text = match commits.row(row) {
                    CommitRow::Branch(label) => format!("branch {label}"),
                    CommitRow::LocalChanges => LOCAL_CHANGES.to_owned(),
                    CommitRow::Commit(commit) => commit.hash.clone(),
                };
                (commits.indent(row), text)
            })
            .collect()
    }

    #[test]
    fn branch_with_its_slices_under_it() {
        let commits = Commits::new(Some("feature".into()), own(&["a", "b"]));
        assert_eq!(
            subjects(&commits),
            [
                (0, "branch feature".into()),
                (1, LOCAL_CHANGES.into()),
                (1, "a".into()),
                (1, "b".into()),
            ]
        );
        assert!(commits.is_selected(0) && commits.is_cursor(0));
        let detached = Commits::new(None, own(&["a"]));
        assert!(matches!(detached.row(0), CommitRow::Branch(BRANCH)));
    }

    #[test]
    fn without_own_commits_local_changes_and_the_earlier_ones() {
        let mut commits = Commits::new(Some("main".into()), earlier(&["a", "b"]));
        assert_eq!(
            subjects(&commits),
            [(0, LOCAL_CHANGES.into()), (0, "a".into()), (0, "b".into()),]
        );
        assert!(commits.is_selected(0) && commits.is_cursor(0));
        assert_eq!(commits.select(2), Some(Selected::Commit("b".into())));
        let empty = Commits::new(Some("main".into()), History::default());
        assert_eq!(subjects(&empty), [(0, LOCAL_CHANGES.into())]);
    }

    #[test]
    fn selects_only_on_change() {
        let mut commits = Commits::new(None, own(&["a", "b"]));
        assert_eq!(commits.select(0), None);
        commits.move_cursor(true);
        assert_eq!(commits.move_cursor(true), 2);
        assert_eq!(
            commits.select_at_cursor(),
            Some(Selected::Commit("a".into()))
        );
        assert_eq!(commits.select_at_cursor(), None);
        assert!(commits.is_selected(2) && commits.is_cursor(2));
    }

    #[test]
    fn steps_from_the_selection() {
        let mut commits = Commits::new(None, own(&["a", "b"]));
        commits.move_cursor(true);
        commits.move_cursor(true);
        assert_eq!(commits.row_next_to_selected(true), 1);
        assert_eq!(commits.row_next_to_selected(false), 0);
        commits.select(3);
        assert_eq!(commits.row_next_to_selected(true), 3);
    }

    #[test]
    fn cursor_and_selection_survive_a_read() {
        let mut commits = Commits::new(None, own(&["a", "b"]));
        commits.select(3);
        commits.move_cursor(false);
        // A new commit on top moves the rows
        commits.set_commits(None, own(&["c", "a", "b"]), Selected::Commit("b".into()));
        assert!(commits.is_selected(4) && commits.is_cursor(3));
    }

    #[test]
    fn a_commit_that_is_gone_gives_branch() {
        let mut commits = Commits::new(None, own(&["a"]));
        commits.move_cursor(true);
        commits.move_cursor(true);
        commits.set_commits(None, own(&["b"]), Selected::Branch);
        assert!(commits.is_cursor(0));
        // Without own commits Branch is hidden, so Local changes instead
        commits.set_commits(None, earlier(&["b"]), Selected::LocalChanges);
        assert!(commits.is_cursor(0) && commits.is_selected(0));
    }

    #[test]
    fn selection_within_the_commits() {
        let commits = own(&["a"]);
        let a = Selected::Commit("a".into());
        assert_eq!(a.clone().within(&commits), a);
        assert_eq!(
            Selected::Commit("gone".into()).within(&commits),
            Selected::Branch
        );
        assert_eq!(Selected::Branch.within(&commits), Selected::Branch);
        assert_eq!(
            Selected::LocalChanges.within(&commits),
            Selected::LocalChanges
        );
        // Without own commits, an earlier commit stays and Branch is gone
        assert_eq!(a.clone().within(&earlier(&["a"])), a);
        assert_eq!(
            Selected::Branch.within(&earlier(&["a"])),
            Selected::LocalChanges
        );
        assert_eq!(a.within(&History::default()), Selected::LocalChanges);
    }
}
