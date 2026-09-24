//! Endrede filer og treet de vises i. Vet ingenting om git eller GUI.

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
