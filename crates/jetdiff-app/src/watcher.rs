//! Følger en mappe og samler endrede stier til det har vært stille en stund. UI-et spør med
//! `take` hver `POLL`, fra sin egen løkke.

use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// Et lagringssteg gir mange hendelser tett etter hverandre.
const QUIET: Duration = Duration::from_millis(300);
/// Melder fra selv om noe skriver hele tiden, som en build.
const MAX_WAIT: Duration = Duration::from_secs(2);
/// Hvor ofte UI-et bør kalle `take`.
pub const POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct Pending {
    paths: Vec<PathBuf>,
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

pub struct Watcher {
    root: PathBuf,
    pending: Arc<Mutex<Pending>>,
    _watcher: RecommendedWatcher,
}

impl Watcher {
    /// `.git` ignoreres, siden git selv skriver der når vi leser.
    pub fn new(root: &Path) -> Result<Self, String> {
        let pending = Arc::new(Mutex::new(Pending::default()));
        let sink = pending.clone();
        let git_dir = root.join(".git");
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            // inotify melder lesing, og vår egen lesing ville gitt en ny reload
            if event.kind.is_access() {
                return;
            }
            let paths: Vec<_> = event
                .paths
                .into_iter()
                .filter(|p| !p.starts_with(&git_dir))
                .collect();
            if paths.is_empty() {
                return;
            }
            let now = Instant::now();
            let mut p = sink.lock().unwrap();
            p.paths.extend(paths);
            p.first.get_or_insert(now);
            p.last = Some(now);
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.to_owned(),
            pending,
            _watcher: watcher,
        })
    }

    /// Stiene som er endret, relative til roten, når det har vært stille lenge nok.
    pub fn take(&self) -> Option<Vec<String>> {
        let paths = {
            let mut p = self.pending.lock().unwrap();
            if !p.ready(Instant::now()) {
                return None;
            }
            mem::take(&mut *p).paths
        };
        Some(relative(&self.root, paths))
    }
}

/// Med `/` som skille, som git skriver dem.
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
    fn venter_til_det_er_stille() {
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
    fn stier_relative_til_roten() {
        let root = Path::new("/r");
        let paths = vec![
            "/r/b/x.rs".into(),
            "/annet/y".into(),
            "/r/a.rs".into(),
            "/r/a.rs".into(),
        ];
        assert_eq!(relative(root, paths), ["a.rs", "b/x.rs"]);
    }
}
