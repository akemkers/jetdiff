//! Branch: everything on the current branch that is not on its base branch, and the branch's own
//! commits. On the base branch itself without own commits, the newest commits on HEAD instead.

use std::path::Path;
use std::thread;

use crate::{History, Repo, empty_tree, git, parse_log, toplevel, verify};

/// The commit list never shows more, so an old base branch doesn't give thousands.
const MAX_COMMITS: usize = 50;

#[derive(Clone)]
pub(crate) enum BranchLog {
    /// The branch's own commits, as a revision range.
    Own(String),
    /// On the base branch, which has none of its own.
    Earlier,
}

impl Repo {
    /// Branch, with files git doesn't track yet, against the merge base with the base branch. Without a
    /// base branch it is the same as Local changes.
    pub fn branch(dir: &Path) -> Result<Self, String> {
        // Each git call costs a process, so the independent ones run at the same time
        let (root, head, base, name) = thread::scope(|s| {
            let head = s.spawn(|| verify(dir, "HEAD"));
            let base = s.spawn(|| base_branch(dir));
            let name = s.spawn(|| current_branch(dir));
            let root = toplevel(dir);
            let (head, base, name) = (head.join(), base.join(), name.join());
            (root, head.unwrap(), base.unwrap(), name.unwrap())
        });
        let root = root?;
        let Some(head) = head else {
            let left_side = empty_tree(&root)?;
            return Ok(Self {
                root,
                left_side,
                right_side: None,
                can_rollback: false,
                branch_log: None,
                branch_name: name,
            });
        };
        let merge_base = base.as_deref().and_then(|base| {
            let out = git(&root, &["merge-base", "HEAD", base]).ok()?;
            Some(out.trim_end().to_owned())
        });
        // origin/main is the base for main, the branch that tracks it
        let on_base = base
            .as_deref()
            .map(|base| base.strip_prefix("origin/").unwrap_or(base))
            .is_some_and(|base| name.as_deref() == Some(base));
        let branch_log = match merge_base
            .as_ref()
            .filter(|merge_base| **merge_base != head)
        {
            Some(merge_base) => Some(BranchLog::Own(format!("{merge_base}..HEAD"))),
            None if on_base => Some(BranchLog::Earlier),
            None => None,
        };
        Ok(Self {
            root,
            left_side: merge_base.unwrap_or(head),
            right_side: None,
            can_rollback: false,
            branch_log,
            branch_name: name,
        })
    }

    /// Newest first. Empty for the other sources.
    pub fn commits(&self) -> Result<History, String> {
        let (range, own) = match &self.branch_log {
            None => return Ok(History::default()),
            Some(BranchLog::Own(range)) => (range.as_str(), true),
            Some(BranchLog::Earlier) => ("HEAD", false),
        };
        let max = format!("-n{MAX_COMMITS}");
        let args = ["log", "-z", "--format=%H%x1f%ct%x1f%s", &max, range, "--"];
        Ok(History {
            commits: parse_log(&git(&self.root, &args)?),
            own,
        })
    }

    /// `None` when HEAD is detached, and for the other sources.
    pub fn branch_name(&self) -> Option<String> {
        self.branch_name.clone()
    }
}

fn current_branch(dir: &Path) -> Option<String> {
    let out = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    Some(out.trim_end().to_owned())
}

/// `origin` first, so the commit list is the same as in the PR. Local main is often behind.
fn base_branch(root: &Path) -> Option<String> {
    let remote = git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()
    .map(|branch| branch.trim_end().to_owned());
    [remote, Some("main".into()), Some("master".into())]
        .into_iter()
        .flatten()
        .find(|branch| verify(root, branch).is_some())
}

#[cfg(test)]
mod tests {
    use jetdiff_files::FileStatus;

    use crate::Repo;
    use crate::temp_repo::TempRepo;
    use crate::tests::summary;

    /// The subjects, and whether they are the branch's own commits.
    fn subjects(temp_repo: &TempRepo) -> (Vec<String>, bool) {
        let history = Repo::branch(&temp_repo.0).unwrap().commits().unwrap();
        let subjects = history.commits.into_iter().map(|commit| commit.subject);
        (subjects.collect(), history.own)
    }

    #[test]
    fn branch_against_the_merge_base() {
        let temp_repo = TempRepo::new("branch-merge-base");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        temp_repo.git(&["checkout", "-q", "-b", "feature"]);
        temp_repo.write("b.rs", "b\n");
        temp_repo.commit();
        // Merged into main after the branch was made, so not part of it
        temp_repo.git(&["checkout", "-q", "main"]);
        temp_repo.write("main.rs", "main\n");
        temp_repo.commit();
        temp_repo.git(&["checkout", "-q", "feature"]);
        temp_repo.write("a.rs", "changed\n");
        temp_repo.write("new.rs", "new\n");
        let repo = Repo::branch(&temp_repo.0).unwrap();
        let files = repo.changed_files().unwrap();
        assert_eq!(
            summary(&files),
            [
                (FileStatus::Modified, "a.rs", None),
                (FileStatus::Added, "b.rs", None),
                (FileStatus::Added, "new.rs", None),
            ]
        );
        let a = files.iter().find(|file| file.path == "a.rs").unwrap();
        assert_eq!(repo.read(a).unwrap(), ["a\n", "changed\n"]);
        assert!(repo.reads_working_tree() && !repo.can_rollback());
    }

    #[test]
    fn commits_against_origin_ahead_of_local_main() {
        let temp_repo = TempRepo::new("origin");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        // Someone else's change is merged into origin/main, but local main is not pulled
        temp_repo.git(&["checkout", "-q", "-b", "other"]);
        temp_repo.write("c.rs", "c\n");
        temp_repo.commit_with("theirs");
        temp_repo.origin_main();
        temp_repo.git(&["checkout", "-q", "-b", "feature"]);
        temp_repo.write("b.rs", "b\n");
        temp_repo.commit_with("mine");
        assert_eq!(subjects(&temp_repo), (vec!["mine".into()], true));
    }

    #[test]
    fn main_with_unpushed_commits() {
        let temp_repo = TempRepo::new("unpushed");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit_with("pushed");
        temp_repo.origin_main();
        // Everything pushed, so main has none of its own and shows the newest
        assert_eq!(subjects(&temp_repo), (vec!["pushed".into()], false));
        temp_repo.write("b.rs", "b\n");
        temp_repo.commit_with("unpushed");
        assert_eq!(subjects(&temp_repo), (vec!["unpushed".into()], true));
    }

    #[test]
    fn own_commits_or_on_the_base_branch_the_newest() {
        let temp_repo = TempRepo::new("commits");
        assert_eq!(subjects(&temp_repo), (Vec::new(), false));
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit_with("first");
        temp_repo.write("a.rs", "b\n");
        temp_repo.git(&["commit", "-q", "-am", "second\n\nmore text"]);
        let earlier = vec!["second".to_owned(), "first".to_owned()];
        assert_eq!(subjects(&temp_repo), (earlier, false));
        // A new branch has no history of its own to show
        temp_repo.git(&["checkout", "-q", "-b", "feature"]);
        assert_eq!(subjects(&temp_repo), (Vec::new(), false));
        temp_repo.write("a.rs", "c\n");
        temp_repo.git(&["commit", "-q", "-am", "on the branch"]);
        assert_eq!(subjects(&temp_repo), (vec!["on the branch".into()], true));
    }

    #[test]
    fn branch_without_own_commits_is_local_changes() {
        let temp_repo = TempRepo::new("no-own-commits");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        temp_repo.write("a.rs", "b\n");
        let files = Repo::branch(&temp_repo.0).unwrap().changed_files().unwrap();
        assert_eq!(summary(&files), [(FileStatus::Modified, "a.rs", None)]);
    }

    #[test]
    fn branch_name_or_none_when_detached() {
        let temp_repo = TempRepo::new("branch-name");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        temp_repo.git(&["checkout", "-q", "-b", "feature/x"]);
        let name = |temp_repo: &TempRepo| Repo::branch(&temp_repo.0).unwrap().branch_name();
        assert_eq!(name(&temp_repo).as_deref(), Some("feature/x"));
        temp_repo.git(&["checkout", "-q", "--detach"]);
        assert_eq!(name(&temp_repo), None);
    }
}
