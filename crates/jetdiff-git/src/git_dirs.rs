//! Where git keeps HEAD and the branches, so the watcher can tell when HEAD moves. Which paths mean
//! that: HEAD itself, the current branch, and packed refs, since the branch can move there. The rest
//! is git's own bookkeeping, also when we read.

use std::path::{Path, PathBuf};

use crate::{Repo, git};

/// In a worktree, HEAD is in the worktree's own git folder and the branches in the common one. In a
/// repo they are the same `.git`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDirs {
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl GitDirs {
    /// The folders to watch for HEAD to move. The common one holds the git folder of a worktree.
    pub fn folders(&self) -> [&Path; 2] {
        [&self.common_dir, &self.git_dir]
    }

    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.git_dir) || path.starts_with(&self.common_dir)
    }

    /// Read when HEAD's own file can't tell, as for a commit on a branch, which only moves the branch.
    pub fn head_moved(&self, paths: &[PathBuf]) -> bool {
        let head = std::fs::read_to_string(self.head()).unwrap_or_default();
        paths
            .iter()
            .any(|path| self.moves_head(path, head_ref(&head)))
    }

    /// `head_ref` is the branch HEAD points to, `None` when it is detached.
    fn moves_head(&self, path: &Path, head_ref: Option<&str>) -> bool {
        if path == self.head() {
            return true;
        }
        head_ref.is_some_and(|head_ref| {
            path == self.common_dir.join("packed-refs") || path == self.common_dir.join(head_ref)
        })
    }

    fn head(&self) -> PathBuf {
        self.git_dir.join("HEAD")
    }
}

/// From the content of `HEAD`, like `ref: refs/heads/feature`. A hash alone is a detached HEAD.
fn head_ref(head: &str) -> Option<&str> {
    head.strip_prefix("ref: ").map(str::trim_end)
}

impl Repo {
    pub fn git_dirs(&self) -> Result<GitDirs, String> {
        let args = [
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
        ];
        let out = git(self.root(), &args)?;
        let mut lines = out.lines().map(PathBuf::from);
        match (lines.next(), lines.next()) {
            (Some(git_dir), Some(common_dir)) => Ok(GitDirs {
                git_dir,
                common_dir,
            }),
            _ => Err(format!("unexpected answer from git: {out}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temp_repo::TempRepo;

    fn git_dirs(git_dir: &str, common_dir: &str) -> GitDirs {
        GitDirs {
            git_dir: PathBuf::from(git_dir),
            common_dir: PathBuf::from(common_dir),
        }
    }

    #[test]
    fn worktree_has_its_own_git_dir() {
        let temp_repo = TempRepo::new("git-dirs");
        temp_repo.write("a.rs", "a\n");
        temp_repo.commit();
        let main = Repo::local_changes(&temp_repo.0)
            .unwrap()
            .git_dirs()
            .unwrap();
        assert_eq!(main.git_dir, main.common_dir);
        assert!(main.git_dir.ends_with(".git"));

        let worktree = temp_repo.0.join("wt");
        let worktree_path = worktree.to_str().unwrap();
        temp_repo.git(&["worktree", "add", "-q", "-b", "feature", worktree_path]);
        let dirs = Repo::local_changes(&worktree).unwrap().git_dirs().unwrap();
        assert_eq!(dirs.common_dir, main.common_dir);
        assert_eq!(dirs.git_dir, main.common_dir.join("worktrees/wt"));
    }

    #[test]
    fn head_and_the_current_branch_in_a_repo() {
        let dirs = git_dirs("/r/.git", "/r/.git");
        let moved = |path: &str| dirs.moves_head(Path::new(path), Some("refs/heads/feature/x"));
        assert!(moved("/r/.git/HEAD"));
        assert!(moved("/r/.git/refs/heads/feature/x"));
        assert!(moved("/r/.git/packed-refs"));
        assert!(!moved("/r/.git/refs/heads/main"));
        assert!(!moved("/r/.git/HEAD.lock"));
        assert!(!moved("/r/.git/index"));
        assert!(!moved("/r/.git/logs/HEAD"));
        assert!(!moved("/r/.git/logs/refs/heads/feature/x"));
    }

    #[test]
    fn head_in_the_worktree_s_own_git_dir() {
        let dirs = git_dirs("/r/.git/worktrees/wt", "/r/.git");
        let moved = |path: &str| dirs.moves_head(Path::new(path), Some("refs/heads/feature"));
        assert!(moved("/r/.git/worktrees/wt/HEAD"));
        assert!(moved("/r/.git/refs/heads/feature"));
        assert!(moved("/r/.git/packed-refs"));
        // The repo's own HEAD belongs to another worktree
        assert!(!moved("/r/.git/HEAD"));
        assert!(!moved("/r/.git/worktrees/wt/index"));
    }

    #[test]
    fn detached_head_has_no_branch() {
        let dirs = git_dirs("/r/.git", "/r/.git");
        let moved = |path: &str| dirs.moves_head(Path::new(path), None);
        assert!(moved("/r/.git/HEAD"));
        assert!(!moved("/r/.git/packed-refs"));
        assert!(!moved("/r/.git/refs/heads/main"));
    }

    #[test]
    fn the_ref_head_points_to() {
        assert_eq!(
            head_ref("ref: refs/heads/feature\n"),
            Some("refs/heads/feature")
        );
        assert_eq!(head_ref("4b825dc642cb6eb9a060e54bf8d69288fbee4904\n"), None);
    }
}
