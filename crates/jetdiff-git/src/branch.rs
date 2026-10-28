//! Branch: everything on the current branch that is not on its base branch, and the branch's own commits.

use std::path::Path;
use std::thread;

use crate::{Commit, Repo, empty_tree, git, parse_log, toplevel, verify};

/// The commit list never shows more, so an old base branch doesn't give thousands.
const MAX_COMMITS: usize = 50;

impl Repo {
    /// Branch, with files git doesn't track yet, against the merge base with the base branch. Without a
    /// base branch it is the same as Local changes.
    pub fn branch(dir: &Path) -> Result<Self, String> {
        // Each git call costs a process, so the independent ones run at the same time
        let (root, head, base) = thread::scope(|s| {
            let head = s.spawn(|| verify(dir, "HEAD"));
            let base = s.spawn(|| base_branch(dir));
            (toplevel(dir), head.join().unwrap(), base.join().unwrap())
        });
        let root = root?;
        let Some(head) = head else {
            let left_side = empty_tree(&root)?;
            return Ok(Self {
                root,
                left_side,
                right_side: None,
                can_rollback: false,
                history: None,
            });
        };
        let merge_base = base.and_then(|base| {
            let out = git(&root, &["merge-base", "HEAD", &base]).ok()?;
            Some(out.trim_end().to_owned())
        });
        let history = merge_base
            .as_ref()
            .filter(|merge_base| **merge_base != head)
            .map(|merge_base| format!("{merge_base}..HEAD"));
        Ok(Self {
            root,
            left_side: merge_base.unwrap_or(head),
            right_side: None,
            can_rollback: false,
            history,
        })
    }

    /// The branch's own commits, newest first. Empty for the other sources.
    pub fn commits(&self) -> Result<Vec<Commit>, String> {
        let Some(history) = &self.history else {
            return Ok(Vec::new());
        };
        let max = format!("-n{MAX_COMMITS}");
        let args = ["log", "-z", "--format=%H%x1f%ct%x1f%s", &max, history, "--"];
        Ok(parse_log(&git(&self.root, &args)?))
    }

    /// `None` when HEAD is detached.
    pub fn branch_name(&self) -> Option<String> {
        let out = git(&self.root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
        Some(out.trim_end().to_owned())
    }
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

    fn subjects(temp_repo: &TempRepo) -> Vec<String> {
        let repo = Repo::branch(&temp_repo.0).unwrap();
        repo.commits()
            .unwrap()
            .into_iter()
            .map(|commit| commit.subject)
            .collect()
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
        assert_eq!(subjects(&temp_repo), ["mine"]);
    }

    #[test]
    fn main_with_unpushed_commits() {
        let temp_repo = TempRepo::new("unpushed");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit_with("pushed");
        temp_repo.origin_main();
        temp_repo.write("b.rs", "b\n");
        temp_repo.commit_with("unpushed");
        assert_eq!(subjects(&temp_repo), ["unpushed"]);
    }

    #[test]
    fn only_the_branch_s_own_commits() {
        let temp_repo = TempRepo::new("commits");
        assert!(subjects(&temp_repo).is_empty());
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit_with("first");
        temp_repo.write("a.rs", "b\n");
        temp_repo.git(&["commit", "-q", "-am", "second\n\nmore text"]);
        assert!(subjects(&temp_repo).is_empty());
        temp_repo.git(&["checkout", "-q", "-b", "feature"]);
        temp_repo.write("a.rs", "c\n");
        temp_repo.git(&["commit", "-q", "-am", "on the branch"]);
        assert_eq!(subjects(&temp_repo), ["on the branch"]);
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
