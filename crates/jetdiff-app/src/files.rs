//! Filtreet. Valgt fil eies av den som bruker treet.

use std::rc::Rc;

use jetdiff_changes::{ChangeTree, FileChange, Row, RowKind};

use crate::step;

pub struct FileTree {
    changes: Rc<[FileChange]>,
    tree: ChangeTree,
    rows: Rc<[Row]>,
    cursor: usize,
}

impl FileTree {
    pub fn new(changes: Rc<[FileChange]>) -> Self {
        let tree = ChangeTree::new(&changes);
        Self {
            rows: tree.rows().into(),
            changes,
            tree,
            cursor: 0,
        }
    }

    pub fn changes(&self) -> &Rc<[FileChange]> {
        &self.changes
    }

    pub fn rows(&self) -> &Rc<[Row]> {
        &self.rows
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_changes(&mut self, changes: Rc<[FileChange]>) {
        self.tree.rebuild(&changes);
        self.rows = self.tree.rows().into();
        self.changes = changes;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }

    pub fn first(&self) -> Option<usize> {
        self.tree.first()
    }

    pub fn next_file(&self, file: usize, forward: bool) -> Option<usize> {
        let next = self.tree.step(file, forward);
        (next != file).then_some(next)
    }

    /// Valgt fil blir synlig, også når den er valgt med tastaturet i en lukket mappe.
    pub fn reveal(&mut self, file: usize) -> Option<usize> {
        self.tree.reveal(file, &self.changes);
        self.rows = self.tree.rows().into();
        let row = self
            .rows
            .iter()
            .position(|r| r.kind == RowKind::File(file))?;
        self.cursor = row;
        Some(row)
    }

    pub fn move_cursor(&mut self, down: bool) -> usize {
        self.cursor = step(self.cursor, down, self.rows.len());
        self.cursor
    }

    pub fn open(&mut self, row: usize) -> Option<usize> {
        self.cursor = row;
        match &self.rows[row].kind {
            RowKind::Dir { path, .. } => {
                self.tree.toggle(&path.clone());
                self.rows = self.tree.rows().into();
                None
            }
            &RowKind::File(file) => Some(file),
        }
    }

    pub fn open_cursor(&mut self) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        self.open(self.cursor)
    }
}

pub fn count_label(count: usize) -> String {
    let files = if count == 1 { "fil" } else { "filer" };
    format!("{count} {files}")
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use jetdiff_changes::Status;

    use super::*;

    fn tree(paths: &[&str]) -> FileTree {
        let changes: Vec<_> = paths
            .iter()
            .map(|p| FileChange {
                path: (*p).into(),
                old_path: None,
                status: Status::Modified,
            })
            .collect();
        FileTree::new(changes.into())
    }

    #[test]
    fn mappe_klappes_og_fil_velges() {
        // src/, src/a.rs, b.rs
        let mut t = tree(&["src/a.rs", "b.rs"]);
        assert_eq!(t.rows().len(), 3);
        assert_eq!(t.open(0), None);
        assert_eq!(t.rows().len(), 2);
        assert_eq!(t.open(1), Some(1));
        t.open(0);
        assert_eq!(t.reveal(0), Some(1));
        assert_eq!(t.rows().len(), 3);
    }

    #[test]
    fn neste_fil_stopper_ved_endene() {
        let t = tree(&["a.rs", "b.rs"]);
        assert_eq!(t.next_file(0, true), Some(1));
        assert_eq!(t.next_file(1, true), None);
        assert_eq!(t.next_file(0, false), None);
    }

    #[test]
    fn markøren_holdes_innenfor_nye_rader() {
        let mut t = tree(&["a.rs", "b.rs", "c.rs"]);
        t.move_cursor(true);
        t.move_cursor(true);
        assert_eq!(t.cursor(), 2);
        let changes = tree(&["a.rs"]).changes().clone();
        t.set_changes(changes);
        assert_eq!(t.cursor(), 0);
        assert_eq!(tree(&[]).open_cursor(), None);
    }

    #[test]
    fn etiketter() {
        assert_eq!(count_label(1), "1 fil");
        assert_eq!(count_label(3), "3 filer");
        assert_eq!(file_name("src/a.rs"), "a.rs");
        assert_eq!(file_name("a.rs"), "a.rs");
    }
}
