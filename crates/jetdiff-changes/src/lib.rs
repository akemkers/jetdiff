//! Changed files and the tree they are shown in. Knows nothing about git or the GUI.

use std::collections::BTreeSet;

pub mod tree;

pub use tree::{ChangeTree, Row, RowKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// Paths are relative with `/` as separator, as git writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    /// The path on Left for a rename.
    pub old_path: Option<String>,
    pub status: Status,
}

impl FileChange {
    pub fn left_path(&self) -> &str {
        self.old_path.as_deref().unwrap_or(&self.path)
    }

    /// Where the file came from, relative to the folder it is in now. As in IntelliJ.
    pub fn renamed_from(&self) -> Option<String> {
        let (old_dir, old_name) = split(self.old_path.as_deref()?);
        let (dir, name) = split(&self.path);
        if old_dir == dir {
            return Some(format!("renamed from {old_name}"));
        }
        let (from, to) = (parts(dir), parts(old_dir));
        let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
        let mut rel: Vec<&str> = vec![".."; from.len() - common];
        rel.extend(&to[common..]);
        if old_name != name {
            rel.push(old_name);
        }
        Some(format!("moved from {}", rel.join("/")))
    }
}

/// Folder and file name. The folder is empty for top-level files.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn parts(dir: &str) -> Vec<&str> {
    dir.split('/').filter(|p| !p.is_empty()).collect()
}

/// The number of files in the list, before or after, that changed on disk or were added,
/// disappeared or got a new file status. `paths` are the changed paths.
pub fn touched(old: &[FileChange], new: &[FileChange], paths: &[String]) -> usize {
    let listed = |p: &str| {
        old.iter()
            .chain(new)
            .any(|c| c.path == p || c.old_path.as_deref() == Some(p))
    };
    let mut touched: BTreeSet<&str> = paths
        .iter()
        .map(String::as_str)
        .filter(|p| listed(p))
        .collect();
    touched.extend(
        old.iter()
            .filter(|c| !new.contains(c))
            .map(|c| c.path.as_str()),
    );
    touched.extend(
        new.iter()
            .filter(|c| !old.contains(c))
            .map(|c| c.path.as_str()),
    );
    touched.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(path: &str, status: Status) -> FileChange {
        FileChange {
            path: path.into(),
            old_path: None,
            status,
        }
    }

    #[test]
    fn counts_changed_files() {
        let old = [
            change("a.rs", Status::Modified),
            change("b.rs", Status::Added),
        ];
        let new = [
            change("a.rs", Status::Modified),
            change("b.rs", Status::Modified),
            change("c.rs", Status::Added),
        ];
        let paths = ["a.rs".into(), "c.rs".into(), "target/x".into()];
        assert_eq!(touched(&old, &new, &paths), 3);
        assert_eq!(touched(&old, &old, &["target/x".into()]), 0);
    }

    #[test]
    fn counts_rename_via_old_path() {
        let renamed = [FileChange {
            old_path: Some("a.rs".into()),
            ..change("b.rs", Status::Renamed)
        }];
        assert_eq!(touched(&renamed, &renamed, &["a.rs".into()]), 1);
    }

    #[test]
    fn where_the_file_came_from() {
        let from = |old: &str, new: &str| {
            FileChange {
                old_path: Some(old.into()),
                ..change(new, Status::Renamed)
            }
            .renamed_from()
            .unwrap()
        };
        assert_eq!(from("src/a.rs", "src/b.rs"), "renamed from a.rs");
        assert_eq!(from("a.rs", "b.rs"), "renamed from a.rs");
        assert_eq!(from("src/a.rs", "src/new/a.rs"), "moved from ..");
        assert_eq!(from("src/new/a.rs", "src/a.rs"), "moved from new");
        assert_eq!(from("a.rs", "src/b.rs"), "moved from ../a.rs");
        assert_eq!(from("lib/x/a.rs", "src/a.rs"), "moved from ../lib/x");
        assert_eq!(change("a.rs", Status::Modified).renamed_from(), None);
    }
}
