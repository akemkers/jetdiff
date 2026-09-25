//! Endrede filer og treet de vises i. Vet ingenting om git eller GUI.

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

/// Stier er relative med `/` som skille, som git skriver dem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    /// Stien på venstre side ved rename.
    pub old_path: Option<String>,
    pub status: Status,
}

impl FileChange {
    pub fn left_path(&self) -> &str {
        self.old_path.as_deref().unwrap_or(&self.path)
    }
}

/// Antall filer i lista, før eller etter, som er endret på disk eller har kommet til,
/// forsvunnet eller fått ny status. `paths` er de endrede stiene.
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
    fn teller_filer_som_er_endret() {
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
    fn teller_rename_via_gammel_sti() {
        let renamed = [FileChange {
            old_path: Some("a.rs".into()),
            ..change("b.rs", Status::Renamed)
        }];
        assert_eq!(touched(&renamed, &renamed, &["a.rs".into()]), 1);
    }
}
