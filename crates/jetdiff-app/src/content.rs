//! From git content to `FileDiff`.

use jetdiff_files::{ChangedFile, FileStatus};
use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Language};

use crate::notification::Notification;

/// The notification is pinned, since it is about the file being shown.
pub fn build_diff(
    file: &ChangedFile,
    content: Result<[String; 2], String>,
) -> (FileDiff, Option<Notification>) {
    match content {
        Err(e) => (empty_diff(), Some(Notification::warning(e))),
        Ok([l, r]) if l.contains('\0') || r.contains('\0') => (
            empty_diff(),
            Some(Notification::warning("Binary file can't be shown")),
        ),
        Ok([l, r]) => match FileDiff::new(&l, &r, ComparisonPolicy::Default) {
            Ok(mut diff) => {
                let lang = Language::from_path(&file.path)
                    .or_else(|| Language::from_path(file.left_side_path()));
                if let Some(lang) = lang {
                    diff.highlight_syntax(lang);
                }
                // When moved, or with only the file mode changed, the diff would otherwise be empty without explanation
                let identical =
                    l == r && !matches!(file.status, FileStatus::Added | FileStatus::Deleted);
                (
                    diff,
                    identical.then(|| Notification::info("The content is identical")),
                )
            }
            Err(_) => (
                empty_diff(),
                Some(Notification::warning("The file is too big to show")),
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

    fn pinned(file: &ChangedFile, content: Result<[String; 2], String>) -> Option<Notification> {
        build_diff(file, content).1
    }

    #[test]
    fn pinned_notification_when_diff_cannot_be_shown() {
        let m = changed_file("a.rs", FileStatus::Modified);
        let text = |l: &str, r: &str| Ok([l.to_owned(), r.to_owned()]);
        assert_eq!(pinned(&m, text("a\n", "b\n")), None);
        assert_eq!(
            pinned(&m, text("a\n", "a\n")),
            Some(Notification::info("The content is identical"))
        );
        assert_eq!(
            pinned(&m, text("a\0", "b")),
            Some(Notification::warning("Binary file can't be shown"))
        );
        assert_eq!(
            pinned(&m, Err("gone".into())),
            Some(Notification::warning("gone"))
        );
        // An empty added file is not «identical»
        assert_eq!(
            pinned(&changed_file("a.rs", FileStatus::Added), text("", "")),
            None
        );
    }
}
