//! Collects changed paths, and whether HEAD moved, until it has been quiet. The UI asks with `take`.

use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jetdiff_git::{GitDirs, Repo};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// A save step gives many events close together.
const QUIET: Duration = Duration::from_millis(300);
/// Reports even if something writes all the time, like a build.
const MAX_WAIT: Duration = Duration::from_secs(2);
pub const POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct Pending {
    paths: Vec<PathBuf>,
    head_moved: bool,
    first: Option<Instant>,
    last: Option<Instant>,
}

impl Pending {
    fn ready(&self, now: Instant) -> bool {
        match (self.first, self.last) {
            (Some(first), Some(last)) => now - last >= QUIET || now - first >= MAX_WAIT,
            _ => false,
        }
    }
}

/// What the watcher saw.
#[derive(Debug, PartialEq, Eq)]
pub enum DiskChange {
    /// Relative to the root, with `/` as separator, as git writes them.
    Files(Vec<String>),
    /// A commit, amend, reset or rebase. Files that changed at the same time are read too.
    HeadMoved,
}

pub struct Watcher {
    root: PathBuf,
    pending: Arc<Mutex<Pending>>,
    _watcher: RecommendedWatcher,
}

impl Watcher {
    /// The git folders are ignored, since git itself writes there when we read, except for HEAD and
    /// the current branch. In a worktree they are outside the root, so they are watched too.
    pub fn new(repo: &Repo) -> Result<Self, String> {
        let root = repo.root().to_owned();
        let git_dirs = repo.git_dirs()?;
        let pending = Arc::new(Mutex::new(Pending::default()));
        let sink = pending.clone();
        let folders = watched(&root, &git_dirs);
        // The `.git` file of a worktree
        let dot_git = root.join(".git");
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            // inotify reports reads, and our own reading would cause a new autoreload
            if event.kind.is_access() {
                return;
            }
            let (git_paths, paths): (Vec<_>, Vec<_>) = event
                .paths
                .into_iter()
                .partition(|path| path.starts_with(&dot_git) || git_dirs.contains(path));
            let head_moved = !git_paths.is_empty() && git_dirs.head_moved(&git_paths);
            if paths.is_empty() && !head_moved {
                return;
            }
            let now = Instant::now();
            let mut pending = sink.lock().unwrap();
            pending.paths.extend(paths);
            pending.head_moved |= head_moved;
            pending.first.get_or_insert(now);
            pending.last = Some(now);
        })
        .map_err(|e| e.to_string())?;
        for folder in folders {
            watcher
                .watch(&folder, RecursiveMode::Recursive)
                .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            root,
            pending,
            _watcher: watcher,
        })
    }

    pub fn take(&self) -> Option<DiskChange> {
        let pending = {
            let mut pending = self.pending.lock().unwrap();
            if !pending.ready(Instant::now()) {
                return None;
            }
            mem::take(&mut *pending)
        };
        Some(if pending.head_moved {
            DiskChange::HeadMoved
        } else {
            DiskChange::Files(relative(&self.root, pending.paths))
        })
    }
}

/// The root, and the git folders that are not inside a folder already watched.
fn watched(root: &Path, git_dirs: &GitDirs) -> Vec<PathBuf> {
    let mut folders = vec![root.to_owned()];
    for folder in git_dirs.folders() {
        if !folders.iter().any(|watched| folder.starts_with(watched)) {
            folders.push(folder.to_owned());
        }
    }
    folders
}

/// With `/` as separator, as git writes them.
fn relative(root: &Path, paths: Vec<PathBuf>) -> Vec<String> {
    let mut rel: Vec<String> = paths
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok())
        .filter_map(|p| p.to_str())
        .map(|p| p.replace(std::path::MAIN_SEPARATOR, "/"))
        .collect();
    rel.sort();
    rel.dedup();
    rel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_until_quiet() {
        let t0 = Instant::now();
        let mut p = Pending::default();
        assert!(!p.ready(t0));
        p.first = Some(t0);
        p.last = Some(t0);
        assert!(!p.ready(t0 + QUIET / 2));
        assert!(p.ready(t0 + QUIET));
        p.last = Some(t0 + MAX_WAIT - QUIET / 2);
        assert!(!p.ready(t0 + MAX_WAIT - POLL));
        assert!(p.ready(t0 + MAX_WAIT));
    }

    #[test]
    fn paths_relative_to_root() {
        let root = Path::new("/r");
        let paths = vec![
            "/r/b/x.rs".into(),
            "/other/y".into(),
            "/r/a.rs".into(),
            "/r/a.rs".into(),
        ];
        assert_eq!(relative(root, paths), ["a.rs", "b/x.rs"]);
    }
}
