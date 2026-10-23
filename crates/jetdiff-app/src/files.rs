//! The file tree's state: its rows and cursor. The selected file is owned by whoever uses the tree.

use std::rc::Rc;

use jetdiff_files::{ChangedFile, Row, RowKind, Tree};

use crate::step;

pub struct Files {
    changed_files: Rc<[ChangedFile]>,
    tree: Tree,
    rows: Rc<[Row]>,
    cursor: usize,
}

impl Files {
    pub fn new(changed_files: Rc<[ChangedFile]>) -> Self {
        let tree = Tree::new(&changed_files);
        Self {
            rows: tree.rows().into(),
            changed_files,
            tree,
            cursor: 0,
        }
    }

    pub fn changed_files(&self) -> &Rc<[ChangedFile]> {
        &self.changed_files
    }

    pub fn rows(&self) -> &Rc<[Row]> {
        &self.rows
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_changed_files(&mut self, changed_files: Rc<[ChangedFile]>) {
        self.tree.rebuild(&changed_files);
        self.rows = self.tree.rows().into();
        self.changed_files = changed_files;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }

    pub fn first(&self) -> Option<usize> {
        self.tree.first()
    }

    pub fn next_file(&self, file: usize, forward: bool) -> Option<usize> {
        let next = self.tree.step(file, forward);
        (next != file).then_some(next)
    }

    /// The selected file becomes visible, also when it was selected with the keyboard in a closed folder.
    pub fn reveal(&mut self, file: usize) -> Option<usize> {
        self.tree.reveal(file, &self.changed_files);
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

    /// The file on the row. A folder row opens or closes the folder instead, and selects nothing.
    pub fn select(&mut self, row: usize) -> Option<usize> {
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

    pub fn select_at_cursor(&mut self) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        self.select(self.cursor)
    }
}

pub fn count_label(count: usize) -> String {
    let files = if count == 1 { "file" } else { "files" };
    format!("{count} {files}")
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use jetdiff_files::FileStatus;

    use super::*;

    fn files(paths: &[&str]) -> Files {
        let changed_files: Vec<_> = paths
            .iter()
            .map(|path| ChangedFile {
                path: (*path).into(),
                renamed_from: None,
                status: FileStatus::Modified,
            })
            .collect();
        Files::new(changed_files.into())
    }

    #[test]
    fn folder_closes_and_file_is_selected() {
        // src/, src/a.rs, b.rs
        let mut t = files(&["src/a.rs", "b.rs"]);
        assert_eq!(t.rows().len(), 3);
        assert_eq!(t.select(0), None);
        assert_eq!(t.rows().len(), 2);
        assert_eq!(t.select(1), Some(1));
        t.select(0);
        assert_eq!(t.reveal(0), Some(1));
        assert_eq!(t.rows().len(), 3);
    }

    #[test]
    fn next_file_stops_at_the_ends() {
        let t = files(&["a.rs", "b.rs"]);
        assert_eq!(t.next_file(0, true), Some(1));
        assert_eq!(t.next_file(1, true), None);
        assert_eq!(t.next_file(0, false), None);
    }

    #[test]
    fn cursor_stays_within_new_rows() {
        let mut t = files(&["a.rs", "b.rs", "c.rs"]);
        t.move_cursor(true);
        t.move_cursor(true);
        assert_eq!(t.cursor(), 2);
        let changed_files = files(&["a.rs"]).changed_files().clone();
        t.set_changed_files(changed_files);
        assert_eq!(t.cursor(), 0);
        assert_eq!(files(&[]).select_at_cursor(), None);
    }

    #[test]
    fn labels() {
        assert_eq!(count_label(1), "1 file");
        assert_eq!(count_label(3), "3 files");
        assert_eq!(file_name("src/a.rs"), "a.rs");
        assert_eq!(file_name("a.rs"), "a.rs");
    }
}
