//! From git content to `FileDiff`.

use jetdiff_files::{ChangedFile, FileStatus};
use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Language};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub level: Level,
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: Level::Info,
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: Level::Warning,
        }
    }
}

pub fn build_diff(
    file: &ChangedFile,
    content: Result<[String; 2], String>,
) -> (FileDiff, Option<Notice>) {
    match content {
        Err(e) => (empty_diff(), Some(Notice::warning(e))),
        Ok([l, r]) if l.contains('\0') || r.contains('\0') => (
            empty_diff(),
            Some(Notice::warning("Binary file can't be shown")),
        ),
        Ok([l, r]) => match FileDiff::new(&l, &r, ComparisonPolicy::Default) {
            Ok(mut diff) => {
                let lang = Language::from_path(&file.path)
                    .or_else(|| Language::from_path(file.left_path()));
                if let Some(lang) = lang {
                    diff.highlight_syntax(lang);
                }
                // When moved, or with only the file mode changed, the diff would otherwise be empty without explanation
                let identical =
                    l == r && !matches!(file.status, FileStatus::Added | FileStatus::Deleted);
                (
                    diff,
                    identical.then(|| Notice::info("The content is identical")),
                )
            }
            Err(_) => (
                empty_diff(),
                Some(Notice::warning("The file is too big to show")),
            ),
        },
    }
}

pub fn empty_diff() -> FileDiff {
    FileDiff::new("", "", ComparisonPolicy::Default).expect("an empty diff is never too big")
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

    fn notice(file: &ChangedFile, content: Result<[String; 2], String>) -> Option<Notice> {
        build_diff(file, content).1
    }

    #[test]
    fn message_when_diff_cannot_be_shown() {
        let m = changed_file("a.rs", FileStatus::Modified);
        let text = |l: &str, r: &str| Ok([l.to_owned(), r.to_owned()]);
        assert_eq!(notice(&m, text("a\n", "b\n")), None);
        assert_eq!(
            notice(&m, text("a\n", "a\n")),
            Some(Notice::info("The content is identical"))
        );
        assert_eq!(
            notice(&m, text("a\0", "b")),
            Some(Notice::warning("Binary file can't be shown"))
        );
        assert_eq!(
            notice(&m, Err("gone".into())),
            Some(Notice::warning("gone"))
        );
        // An empty added file is not «identical»
        assert_eq!(
            notice(&changed_file("a.rs", FileStatus::Added), text("", "")),
            None
        );
    }
}
