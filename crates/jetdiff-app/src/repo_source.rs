//! A repo source: its changed files, the file being shown, and reading it again.

mod read;
mod show;

use std::path::Path;
use std::rc::Rc;

use jetdiff_files::{ChangedFile, Tree};
use jetdiff_git::Repo;

use crate::actions::RollbackRequest;
use crate::commits::{Commits, Selected};
use crate::content::build_diff;
use crate::notification::Notification;
use crate::watcher::DiskChange;
use read::Trigger;
pub use read::{ReadJob, RepoRead};
pub use show::{Applied, Show, Shown};

#[derive(Clone, PartialEq)]
struct ShownFile {
    file: ChangedFile,
    content: Result<[String; 2], String>,
    /// Part of what is shown, so switching from Branch to Local changes is not `Show::Unchanged`.
    can_rollback: bool,
}

pub struct RepoSource {
    repo: Repo,
    /// What the commit list has selected. A commit or pull request opened directly has no commit list
    /// and is never read again.
    selected: Selected,
    /// What `changed_files` is from. Becomes equal to `selected` when a read is applied.
    last_read_selected: Selected,
    changed_files: Rc<[ChangedFile]>,
    /// What the diff shows now. The same after a read leaves the position alone.
    shown: Option<ShownFile>,
}

impl RepoSource {
    /// A commit or a pull request.
    pub fn new(repo: Repo, changed_files: Vec<ChangedFile>) -> Self {
        Self {
            repo,
            selected: Selected::Branch,
            last_read_selected: Selected::Branch,
            changed_files: changed_files.into(),
            shown: None,
        }
    }

    /// The repo or worktree `dir` is in, with the commit list. Starts on Branch, or on Local changes
    /// when the branch has no commits of its own.
    pub fn open(dir: &Path) -> Result<(Self, Commits), String> {
        let job = ReadJob {
            root: dir.to_owned(),
            selected: Selected::Branch,
            trigger: Trigger::SelectionChanged,
        };
        let RepoRead {
            selected,
            repo,
            changed_files,
            branch_name,
            commits,
            ..
        } = job.run()?;
        let source = Self {
            repo,
            selected: selected.clone(),
            last_read_selected: selected,
            changed_files: changed_files.into(),
            shown: None,
        };
        Ok((source, Commits::new(branch_name, commits)))
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn changed_files(&self) -> &Rc<[ChangedFile]> {
        &self.changed_files
    }

    pub fn shown(&self) -> Option<&ChangedFile> {
        self.shown.as_ref().map(|shown| &shown.file)
    }

    pub fn first(&self) -> Option<usize> {
        Tree::new(&self.changed_files).first()
    }

    pub fn changed_on_disk(&self, change: DiskChange) -> ReadJob {
        self.read(match change {
            DiskChange::Files(paths) => Trigger::FilesChanged(paths),
            DiskChange::HeadMoved => Trigger::HeadMoved,
        })
    }

    /// Shown when the read is applied.
    pub fn select(&mut self, selected: Selected) -> ReadJob {
        self.selected = selected;
        self.read(Trigger::SelectionChanged)
    }

    /// After `rollback`, so the list keeps up.
    pub fn rolled_back(&self) -> ReadJob {
        self.read(Trigger::RolledBack)
    }

    fn read(&self, trigger: Trigger) -> ReadJob {
        ReadJob {
            root: self.repo.root().to_owned(),
            selected: self.selected.clone(),
            trigger,
        }
    }

    pub fn apply(&mut self, read: Result<RepoRead, String>) -> Applied {
        let RepoRead {
            trigger,
            selected,
            repo,
            changed_files,
            branch_name,
            commits,
        } = match read {
            Ok(read) => read,
            Err(error) => {
                self.selected = self.last_read_selected.clone();
                return Applied::Failed {
                    selected: self.selected.clone(),
                    brief: Notification::warning(error),
                };
            }
        };
        // Files are only counted when the working tree is read again as the same source. A commit doesn't change.
        let same_source = selected == self.last_read_selected && repo.reads_working_tree();
        let touched = match &trigger {
            Trigger::FilesChanged(paths) if same_source => {
                jetdiff_files::touched(&self.changed_files, &changed_files, paths)
            }
            Trigger::FilesChanged(_)
            | Trigger::HeadMoved
            | Trigger::SelectionChanged
            | Trigger::RolledBack => 0,
        };
        let shown_path = self.shown().map(|file| file.path.clone());
        self.repo = repo;
        self.selected = selected.clone();
        self.last_read_selected = selected.clone();
        let changed = (*changed_files != *self.changed_files).then(|| {
            self.changed_files = changed_files.into();
            self.changed_files.clone()
        });
        // The selected file is kept as long as it is included
        let file = shown_path
            .and_then(|path| self.changed_files.iter().position(|file| file.path == path))
            .or_else(|| self.first());
        let brief = match touched {
            0 => None,
            1 => Some(Notification::info("1 file changed")),
            n => Some(Notification::info(format!("{n} files changed"))),
        };
        Applied::Done {
            branch_name,
            commits,
            selected,
            changed_files: changed,
            file,
            brief,
        }
    }

    pub fn load(&mut self, file: Option<usize>) -> Show {
        let Some(changed_file) = file.map(|i| self.changed_files[i].clone()) else {
            self.shown = None;
            return Show::Empty {
                pinned: Notification::info("No changed files"),
            };
        };
        let content = self.repo.read(&changed_file);
        self.show(changed_file, content)
    }

    fn show(&mut self, file: ChangedFile, content: Result<[String; 2], String>) -> Show {
        let can_rollback = self.repo.can_rollback();
        let shown = Some(ShownFile {
            file: file.clone(),
            content: content.clone(),
            can_rollback,
        });
        if shown == self.shown {
            return Show::Unchanged;
        }
        let same_file = self
            .shown()
            .is_some_and(|previous| previous.path == file.path);
        self.shown = shown;
        let (diff, pinned) = build_diff(&file, content);
        Show::Diff(Box::new(Shown {
            diff,
            pinned,
            paths: [file.left_side_path().to_owned(), file.path.clone()],
            can_rollback,
            same_file,
        }))
    }

    /// Run `rolled_back` afterwards, so the list keeps up.
    pub fn rollback(&mut self, request: &RollbackRequest) -> Option<Result<Show, String>> {
        let Some(ShownFile {
            file,
            content: Ok([left_side, _]),
            ..
        }) = self.shown.clone()
        else {
            return None;
        };
        Some(
            self.repo
                .rollback(&file, request.lines.clone(), &request.before)
                .map(|after| self.show(file, Ok([left_side, after]))),
        )
    }
}

#[cfg(test)]
mod tests;
