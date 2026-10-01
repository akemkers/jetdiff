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

    /// Hvor fila kom fra, relativt til mappa den ligger i nå. Som i IntelliJ.
    pub fn renamed_from(&self) -> Option<String> {
        let (old_dir, old_name) = split(self.old_path.as_deref()?);
        let (dir, name) = split(&self.path);
        if old_dir == dir {
            return Some(format!("omdøpt fra {old_name}"));
        }
        let (from, to) = (parts(dir), parts(old_dir));
        let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
        let mut rel: Vec<&str> = vec![".."; from.len() - common];
        rel.extend(&to[common..]);
        if old_name != name {
            rel.push(old_name);
        }
        Some(format!("flyttet fra {}", rel.join("/")))
    }
}

/// Mappe og filnavn. Mappa er tom for filer på toppnivå.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn parts(dir: &str) -> Vec<&str> {
    dir.split('/').filter(|p| !p.is_empty()).collect()
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

    #[test]
    fn hvor_fila_kom_fra() {
        let from = |old: &str, new: &str| {
            FileChange {
                old_path: Some(old.into()),
                ..change(new, Status::Renamed)
            }
            .renamed_from()
            .unwrap()
        };
        assert_eq!(from("src/a.rs", "src/b.rs"), "omdøpt fra a.rs");
        assert_eq!(from("a.rs", "b.rs"), "omdøpt fra a.rs");
        assert_eq!(from("src/a.rs", "src/ny/a.rs"), "flyttet fra ..");
        assert_eq!(from("src/ny/a.rs", "src/a.rs"), "flyttet fra ny");
        assert_eq!(from("a.rs", "src/b.rs"), "flyttet fra ../a.rs");
        assert_eq!(from("lib/x/a.rs", "src/a.rs"), "flyttet fra ../lib/x");
        assert_eq!(change("a.rs", Status::Modified).renamed_from(), None);
    }
}
