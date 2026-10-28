//! A git repo in a temporary folder for the tests, removed when dropped.

use std::path::PathBuf;

use crate::git;

pub(crate) struct TempRepo(pub(crate) PathBuf);

impl TempRepo {
    pub(crate) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("jetdiff-git-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "t@t"]);
        repo.git(&["config", "user.name", "t"]);
        repo
    }

    pub(crate) fn git(&self, args: &[&str]) -> String {
        git(&self.0, args).unwrap()
    }

    pub(crate) fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub(crate) fn commit(&self) {
        self.commit_with("c");
    }

    pub(crate) fn commit_with(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// As after a fetch where `origin/main` is at HEAD.
    pub(crate) fn origin_main(&self) {
        self.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        self.git(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]);
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
