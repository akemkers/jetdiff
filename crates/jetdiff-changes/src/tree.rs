//! Treet fra IntelliJ sin `TreeModelBuilder`: mapper før filer, sortert på navn, og mapper
//! med bare én undermappe slått sammen til `a/b/c` (`collapseDirectories`).

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::FileChange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Dir {
        /// Hele stien, brukt som id for sammenklapping.
        path: String,
        name: String,
        count: usize,
        expanded: bool,
    },
    /// Indeks i `changes`.
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
pub struct ChangeTree {
    root: Dir,
    collapsed: HashSet<String>,
    /// Filene i den rekkefølgen treet viser dem, for neste og forrige fil.
    order: Vec<usize>,
}

impl ChangeTree {
    pub fn new(changes: &[FileChange]) -> Self {
        let mut root = Dir::default();
        for (i, c) in changes.iter().enumerate() {
            let mut dir = &mut root;
            let mut parts: Vec<&str> = c.path.split('/').collect();
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
        finish(&mut root, changes);
        for d in &mut root.dirs {
            merge_single_children(d);
        }
        let mut order = Vec::new();
        collect_files(&root, &mut order);
        Self {
            root,
            collapsed: HashSet::new(),
            order,
        }
    }

    /// Nye endringer. Sammenklappede mapper forblir sammenklappet.
    pub fn rebuild(&mut self, changes: &[FileChange]) {
        let collapsed = std::mem::take(&mut self.collapsed);
        *self = Self {
            collapsed,
            ..Self::new(changes)
        };
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.push_rows(&self.root, 0, &mut rows);
        rows
    }

    fn push_rows(&self, dir: &Dir, depth: usize, rows: &mut Vec<Row>) {
        for d in &dir.dirs {
            let expanded = !self.collapsed.contains(&d.path);
            rows.push(Row {
                depth,
                kind: RowKind::Dir {
                    path: d.path.clone(),
                    name: d.name.clone(),
                    count: d.count,
                    expanded,
                },
            });
            if expanded {
                self.push_rows(d, depth + 1, rows);
            }
        }
        rows.extend(dir.files.iter().map(|&f| Row {
            depth,
            kind: RowKind::File(f),
        }));
    }

    pub fn toggle(&mut self, dir_path: &str) {
        if !self.collapsed.remove(dir_path) {
            self.collapsed.insert(dir_path.to_owned());
        }
    }

    pub fn first(&self) -> Option<usize> {
        self.order.first().copied()
    }

    /// Neste eller forrige fil i trerekkefølge. Står stille ved endene.
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

    /// Åpner mappene over en fil, så den synes når den velges med tastaturet.
    pub fn reveal(&mut self, file: usize, changes: &[FileChange]) {
        let path = &changes[file].path;
        let mut end = 0;
        while let Some(i) = path[end..].find('/') {
            end += i;
            self.collapsed.remove(&path[..end]);
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

/// Sorterer og teller filer. Navn sammenlignes uten store og små bokstaver først.
fn finish(dir: &mut Dir, changes: &[FileChange]) -> usize {
    let name = |i: &usize| changes[*i].path.rsplit('/').next().unwrap_or_default();
    dir.files.sort_by(|a, b| by_name(name(a), name(b)));
    dir.dirs.sort_by(|a, b| by_name(&a.name, &b.name));
    dir.count = dir.files.len()
        + dir
            .dirs
            .iter_mut()
            .map(|d| finish(d, changes))
            .sum::<usize>();
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
    use crate::Status;

    fn changes(paths: &[&str]) -> Vec<FileChange> {
        paths
            .iter()
            .map(|p| FileChange {
                path: p.to_string(),
                old_path: None,
                status: Status::Modified,
            })
            .collect()
    }

    /// Rader som `"  navn (antall)"` for mapper og `"  fil"` for filer.
    fn show(tree: &ChangeTree, changes: &[FileChange]) -> Vec<String> {
        tree.rows()
            .iter()
            .map(|r| {
                let indent = "  ".repeat(r.depth);
                match &r.kind {
                    RowKind::Dir { name, count, .. } => format!("{indent}{name} ({count})"),
                    RowKind::File(i) => format!("{indent}{}", changes[*i].path),
                }
            })
            .collect()
    }

    #[test]
    fn mapper_for_filer_og_sortert() {
        let c = changes(&["b.rs", "src/z.rs", "A.md", "src/a.rs", "docs/x.md"]);
        let t = ChangeTree::new(&c);
        assert_eq!(
            show(&t, &c),
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
    fn enkeltbarn_mapper_slas_sammen() {
        let c = changes(&[
            "crates/ui/src/a.rs",
            "crates/ui/src/b.rs",
            "crates/core/c.rs",
        ]);
        let t = ChangeTree::new(&c);
        assert_eq!(
            show(&t, &c),
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
    fn sammenklapping_og_reveal() {
        let c = changes(&["a/b/x.rs", "a/c/y.rs"]);
        let mut t = ChangeTree::new(&c);
        t.toggle("a");
        assert_eq!(show(&t, &c), ["a (2)"]);
        t.reveal(1, &c);
        assert_eq!(show(&t, &c).len(), 5);
    }

    #[test]
    fn rebuild_beholder_sammenklapping() {
        let c = changes(&["a/x.rs", "b/y.rs"]);
        let mut t = ChangeTree::new(&c);
        t.toggle("a");
        let c = changes(&["a/x.rs", "a/z.rs", "b/y.rs"]);
        t.rebuild(&c);
        assert_eq!(show(&t, &c), ["a (2)", "b (1)", "  b/y.rs"]);
    }

    #[test]
    fn tomt_tre() {
        let t = ChangeTree::new(&[]);
        assert_eq!(t.first(), None);
        assert!(t.rows().is_empty());
    }

    #[test]
    fn neste_fil_folger_treet() {
        let c = changes(&["z.rs", "src/a.rs"]);
        let t = ChangeTree::new(&c);
        assert_eq!(t.first(), Some(1));
        assert_eq!(t.step(1, true), 0);
        assert_eq!(t.step(0, true), 0);
        assert_eq!(t.step(0, false), 1);
        assert_eq!(t.step(1, false), 1);
    }
}
