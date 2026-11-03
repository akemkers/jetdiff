//! The diff as the model reads it: each file with its hunks, each hunk with an alias like `h3`
//! and a few unchanged lines around it. Generated files are listed but not sent, and files that
//! can't be shown are listed but not read.

mod generated;

use std::fmt::Write;
use std::rc::Rc;

use jetdiff_files::{ChangedFile, FileStatus};
use jetdiff_git::Repo;
use jetdiff_viewmodel::{FileDiff, Side, UnifiedDiff};

use crate::content::build_diff;
use generated::is_generated;

/// Unchanged lines around each hunk, as in `git diff`.
pub const CONTEXT: usize = 3;
/// About 175k tokens. A larger diff is refused rather than cut, since a walkthrough of half a
/// diff reads as confidently as one of the whole.
const MAX_CHARS: usize = 640_000;

/// A hunk in one of the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HunkRef {
    pub file: usize,
    pub hunk: usize,
}

pub enum FileContent {
    Diff(Box<FileDiff>),
    /// A lock file or other generated code, by its name.
    Generated,
    /// Why it isn't read, like a binary file.
    NotRead(String),
}

pub struct DigestFile {
    pub file: ChangedFile,
    pub content: FileContent,
}

pub struct Digest {
    files: Vec<DigestFile>,
    /// The hunk each alias points to: `h1` is the first.
    hunks: Vec<HunkRef>,
    text: String,
}

impl Digest {
    /// `files` with their content on Left and Right.
    pub fn new(files: Vec<(ChangedFile, Result<[String; 2], String>)>) -> Self {
        let mut digest = Self {
            files: Vec::with_capacity(files.len()),
            hunks: Vec::new(),
            text: String::new(),
        };
        for (file, content) in files {
            let content = if is_generated(&file.path) {
                FileContent::Generated
            } else {
                match build_diff(&file, content) {
                    (_, Some(notification)) => FileContent::NotRead(notification.text),
                    (diff, None) => FileContent::Diff(Box::new(diff)),
                }
            };
            digest.files.push(DigestFile { file, content });
            digest.add_text(digest.files.len() - 1);
        }
        digest
    }

    /// Reads every file from git or disk, so it is run on another thread.
    pub fn read(repo: &Repo, changed_files: &[ChangedFile]) -> Self {
        let files = changed_files
            .iter()
            .map(|file| (file.clone(), repo.read(file)))
            .collect();
        Self::new(files)
    }

    pub fn files(&self) -> &[DigestFile] {
        &self.files
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }

    pub fn is_too_large(&self) -> bool {
        self.text.len() > MAX_CHARS
    }

    /// `None` for an alias the digest doesn't have.
    pub fn hunk(&self, alias: &str) -> Option<HunkRef> {
        let number: usize = alias.strip_prefix('h')?.parse().ok()?;
        self.hunks.get(number.checked_sub(1)?).copied()
    }

    fn add_text(&mut self, file: usize) {
        let DigestFile {
            file: changed_file,
            content,
        } = &self.files[file];
        let FileContent::Diff(diff) = content else {
            return;
        };
        let text = &mut self.text;
        let _ = writeln!(text, "=== {} ({})", changed_file.path, status(changed_file));
        let diff = UnifiedDiff::new(Rc::new(diff.as_ref().clone()));
        for hunk in 0..diff.diff.hunks().len() {
            self.hunks.push(HunkRef { file, hunk });
            let rows = diff.excerpts(&[hunk], CONTEXT).remove(0);
            let rows = &diff.rows()[rows];
            let count = |side: Side| {
                rows.iter()
                    .filter(|row| row.lines[side.select(0, 1)].is_some())
                    .count()
            };
            let start = |side: Side| {
                let first = rows.iter().find_map(|row| row.lines[side.select(0, 1)]);
                first.map_or(0, |line| line + 1)
            };
            let _ = writeln!(
                text,
                "[h{}] @@ -{},{} +{},{} @@",
                self.hunks.len(),
                start(Side::Left),
                count(Side::Left),
                start(Side::Right),
                count(Side::Right),
            );
            for row in rows {
                let sign = match row.lines {
                    [Some(_), Some(_)] => ' ',
                    [Some(_), None] => '-',
                    _ => '+',
                };
                let _ = writeln!(text, "{sign}{}", diff.diff.line(row.side(), row.line()));
            }
        }
    }
}

fn status(file: &ChangedFile) -> String {
    match file.status {
        FileStatus::Added => "added".into(),
        FileStatus::Deleted => "deleted".into(),
        FileStatus::Modified => "modified".into(),
        FileStatus::Renamed => format!("renamed from {}", file.left_side_path()),
    }
}

#[cfg(test)]
mod tests;
