//! The tree from IntelliJ's `TreeModelBuilder`: folders before files, sorted by name, and folders
//! with only one subfolder merged into `a/b/c` (`collapseDirectories`).

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::ChangedFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Dir {
        /// The whole path, used as the id for opening and closing.
        path: String,
        name: String,
        count: usize,
        open: bool,
    },
    /// Index in the changed files.
    File(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub depth: usize,
    pub kind: RowKind,
}

#[derive(Debug, Default)]
struct Dir {
    path: String,
    name: String,
    dirs: Vec<Dir>,
    files: Vec<usize>,
    count: usize,
}

#[derive(Debug)]
pub struct Tree {
    root: Dir,
    closed_folders: HashSet<String>,
    /// The files in the order the tree shows them, for next and previous file.
    order: Vec<usize>,
}

impl Tree {
    pub fn new(files: &[ChangedFile]) -> Self {
        let mut root = Dir::default();
        for (i, file) in files.iter().enumerate() {
            let mut dir = &mut root;
            let mut parts: Vec<&str> = file.path.split('/').collect();
            parts.pop();
            for part in parts {
                let pos = match dir.dirs.iter().position(|d| d.name == part) {
                    Some(p) => p,
                    None => {
                        let path = join(&dir.path, part);
                        dir.dirs.push(Dir {
                            path,
                            name: part.to_owned(),
                            ..Dir::default()
                        });
                        dir.dirs.len() - 1
                    }
                };
                dir = &mut dir.dirs[pos];
            }
            dir.files.push(i);
        }
        finish(&mut root, files);
        for d in &mut root.dirs {
            merge_single_children(d);
        }
        let mut order = Vec::new();
        collect_files(&root, &mut order);
        Self {
            root,
            closed_folders: HashSet::new(),
            order,
        }
    }

    /// New changed files. Closed folders stay closed.
    pub fn rebuild(&mut self, files: &[ChangedFile]) {
        let closed_folders = std::mem::take(&mut self.closed_folders);
        *self = Self {
            closed_folders,
            ..Self::new(files)
        };
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.push_rows(&self.root, 0, &mut rows);
        rows
    }

    fn push_rows(&self, dir: &Dir, depth: usize, rows: &mut Vec<Row>) {
        for d in &dir.dirs {
            let open = !self.closed_folders.contains(&d.path);
            rows.push(Row {
                depth,
                kind: RowKind::Dir {
                    path: d.path.clone(),
                    name: d.name.clone(),
                    count: d.count,
                    open,
                },
            });
            if open {
                self.push_rows(d, depth + 1, rows);
            }
        }
        rows.extend(dir.files.iter().map(|&f| Row {
            depth,
            kind: RowKind::File(f),
        }));
    }

    pub fn toggle(&mut self, dir_path: &str) {
        if !self.closed_folders.remove(dir_path) {
            self.closed_folders.insert(dir_path.to_owned());
        }
    }

    pub fn first(&self) -> Option<usize> {
        self.order.first().copied()
    }

    /// Next or previous file in tree order. Stands still at the ends.
    pub fn step(&self, file: usize, forward: bool) -> usize {
        let Some(i) = self.order.iter().position(|&f| f == file) else {
            return file;
        };
        let j = if forward {
            (i + 1).min(self.order.len() - 1)
        } else {
            i.saturating_sub(1)
        };
        self.order[j]
    }

    /// Opens the folders above a file, so it shows when it is selected with the keyboard.
    pub fn reveal(&mut self, file: usize, files: &[ChangedFile]) {
        let path = &files[file].path;
        let mut end = 0;
        while let Some(i) = path[end..].find('/') {
            end += i;
            self.closed_folders.remove(&path[..end]);
            end += 1;
        }
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

/// Sorts and counts files. Names are compared case-insensitively first.
fn finish(dir: &mut Dir, files: &[ChangedFile]) -> usize {
    let name = |i: &usize| files[*i].path.rsplit('/').next().unwrap_or_default();
    dir.files.sort_by(|a, b| by_name(name(a), name(b)));
    dir.dirs.sort_by(|a, b| by_name(&a.name, &b.name));
    dir.count = dir.files.len() + dir.dirs.iter_mut().map(|d| finish(d, files)).sum::<usize>();
    dir.count
}

fn by_name(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

fn merge_single_children(dir: &mut Dir) {
    while dir.files.is_empty() && dir.dirs.len() == 1 {
        let child = dir.dirs.pop().unwrap();
        dir.name = format!("{}/{}", dir.name, child.name);
        dir.path = child.path;
        dir.dirs = child.dirs;
        dir.files = child.files;
    }
    for d in &mut dir.dirs {
        merge_single_children(d);
    }
}

fn collect_files(dir: &Dir, out: &mut Vec<usize>) {
    for d in &dir.dirs {
        collect_files(d, out);
    }
    out.extend(&dir.files);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileStatus;

    fn changed_files(paths: &[&str]) -> Vec<ChangedFile> {
        paths
            .iter()
            .map(|path| ChangedFile {
                path: path.to_string(),
                renamed_from: None,
                status: FileStatus::Modified,
            })
            .collect()
    }

    /// Rows like `"  name (count)"` for folders and `"  file"` for files.
    fn show(tree: &Tree, files: &[ChangedFile]) -> Vec<String> {
        tree.rows()
            .iter()
            .map(|r| {
                let indent = "  ".repeat(r.depth);
                match &r.kind {
                    RowKind::Dir { name, count, .. } => format!("{indent}{name} ({count})"),
                    RowKind::File(i) => format!("{indent}{}", files[*i].path),
                }
            })
            .collect()
    }

    #[test]
    fn folders_before_files_and_sorted() {
        let files = changed_files(&["b.rs", "src/z.rs", "A.md", "src/a.rs", "docs/x.md"]);
        let tree = Tree::new(&files);
        assert_eq!(
            show(&tree, &files),
            [
                "docs (1)",
                "  docs/x.md",
                "src (2)",
                "  src/a.rs",
                "  src/z.rs",
                "A.md",
                "b.rs"
            ]
        );
    }

    #[test]
    fn single_child_folders_are_merged() {
        let files = changed_files(&[
            "crates/ui/src/a.rs",
            "crates/ui/src/b.rs",
            "crates/core/c.rs",
        ]);
        let tree = Tree::new(&files);
        assert_eq!(
            show(&tree, &files),
            [
                "crates (3)",
                "  core (1)",
                "    crates/core/c.rs",
                "  ui/src (2)",
                "    crates/ui/src/a.rs",
                "    crates/ui/src/b.rs",
            ]
        );
    }

    #[test]
    fn closing_and_reveal() {
        let files = changed_files(&["a/b/x.rs", "a/c/y.rs"]);
        let mut tree = Tree::new(&files);
        tree.toggle("a");
        assert_eq!(show(&tree, &files), ["a (2)"]);
        tree.reveal(1, &files);
        assert_eq!(show(&tree, &files).len(), 5);
    }

    #[test]
    fn rebuild_keeps_folders_closed() {
        let files = changed_files(&["a/x.rs", "b/y.rs"]);
        let mut tree = Tree::new(&files);
        tree.toggle("a");
        let files = changed_files(&["a/x.rs", "a/z.rs", "b/y.rs"]);
        tree.rebuild(&files);
        assert_eq!(show(&tree, &files), ["a (2)", "b (1)", "  b/y.rs"]);
    }

    #[test]
    fn empty_tree() {
        let tree = Tree::new(&[]);
        assert_eq!(tree.first(), None);
        assert!(tree.rows().is_empty());
    }

    #[test]
    fn next_file_follows_the_tree() {
        let files = changed_files(&["z.rs", "src/a.rs"]);
        let tree = Tree::new(&files);
        assert_eq!(tree.first(), Some(1));
        assert_eq!(tree.step(1, true), 0);
        assert_eq!(tree.step(0, true), 0);
        assert_eq!(tree.step(0, false), 1);
        assert_eq!(tree.step(1, false), 1);
    }
}
