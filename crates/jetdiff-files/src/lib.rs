//! Changed files and the tree they are shown in. Knows nothing about git or the GUI.

use std::collections::BTreeSet;

pub mod tree;

pub use tree::{Row, RowKind, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// Paths are relative with `/` as separator, as git writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    /// The path on Left for a rename.
    pub renamed_from: Option<String>,
    pub status: FileStatus,
}

impl ChangedFile {
    pub fn left_side_path(&self) -> &str {
        self.renamed_from.as_deref().unwrap_or(&self.path)
    }

    /// Where the file came from, relative to the folder it is in now. As in IntelliJ.
    pub fn renamed_from_label(&self) -> Option<String> {
        let (from_dir, from_name) = split(self.renamed_from.as_deref()?);
        let (dir, name) = split(&self.path);
        if from_dir == dir {
            return Some(format!("renamed from {from_name}"));
        }
        let (from, to) = (parts(dir), parts(from_dir));
        let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
        let mut relative_path: Vec<&str> = vec![".."; from.len() - common];
        relative_path.extend(&to[common..]);
        if from_name != name {
            relative_path.push(from_name);
        }
        Some(format!("moved from {}", relative_path.join("/")))
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
pub fn touched(previous: &[ChangedFile], current: &[ChangedFile], paths: &[String]) -> usize {
    let listed = |p: &str| {
        previous
            .iter()
            .chain(current)
            .any(|file| file.path == p || file.renamed_from.as_deref() == Some(p))
    };
    let mut touched: BTreeSet<&str> = paths
        .iter()
        .map(String::as_str)
        .filter(|p| listed(p))
        .collect();
    touched.extend(
        previous
            .iter()
            .filter(|file| !current.contains(file))
            .map(|file| file.path.as_str()),
    );
    touched.extend(
        current
            .iter()
            .filter(|file| !previous.contains(file))
            .map(|file| file.path.as_str()),
    );
    touched.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed_file(path: &str, status: FileStatus) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            renamed_from: None,
            status,
        }
    }

    #[test]
    fn counts_changed_files() {
        let previous = [
            changed_file("a.rs", FileStatus::Modified),
            changed_file("b.rs", FileStatus::Added),
        ];
        let current = [
            changed_file("a.rs", FileStatus::Modified),
            changed_file("b.rs", FileStatus::Modified),
            changed_file("c.rs", FileStatus::Added),
        ];
        let paths = ["a.rs".into(), "c.rs".into(), "target/x".into()];
        assert_eq!(touched(&previous, &current, &paths), 3);
        assert_eq!(touched(&previous, &previous, &["target/x".into()]), 0);
    }

    #[test]
    fn counts_rename_via_renamed_from() {
        let renamed = [ChangedFile {
            renamed_from: Some("a.rs".into()),
            ..changed_file("b.rs", FileStatus::Renamed)
        }];
        assert_eq!(touched(&renamed, &renamed, &["a.rs".into()]), 1);
    }

    #[test]
    fn where_the_file_came_from() {
        let label = |from: &str, to: &str| {
            ChangedFile {
                renamed_from: Some(from.into()),
                ..changed_file(to, FileStatus::Renamed)
            }
            .renamed_from_label()
            .unwrap()
        };
        assert_eq!(label("src/a.rs", "src/b.rs"), "renamed from a.rs");
        assert_eq!(label("a.rs", "b.rs"), "renamed from a.rs");
        assert_eq!(label("src/a.rs", "src/new/a.rs"), "moved from ..");
        assert_eq!(label("src/new/a.rs", "src/a.rs"), "moved from new");
        assert_eq!(label("a.rs", "src/b.rs"), "moved from ../a.rs");
        assert_eq!(label("lib/x/a.rs", "src/a.rs"), "moved from ../lib/x");
        assert_eq!(
            changed_file("a.rs", FileStatus::Modified).renamed_from_label(),
            None
        );
    }
}
