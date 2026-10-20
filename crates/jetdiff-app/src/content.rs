//! Fra git-innhold til `FileDiff`.

use jetdiff_changes::{FileChange, Status};
use jetdiff_view::{ComparisonPolicy, FileDiff, Language};

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
    change: &FileChange,
    content: Result<[String; 2], String>,
) -> (FileDiff, Option<Notice>) {
    match content {
        Err(e) => (empty_diff(), Some(Notice::warning(e))),
        Ok([l, r]) if l.contains('\0') || r.contains('\0') => (
            empty_diff(),
            Some(Notice::warning("Binærfil kan ikke vises")),
        ),
        Ok([l, r]) => match FileDiff::new(&l, &r, ComparisonPolicy::Default) {
            Ok(mut diff) => {
                let lang = Language::from_path(&change.path)
                    .or_else(|| Language::from_path(change.left_path()));
                if let Some(lang) = lang {
                    diff.highlight_syntax(lang);
                }
                // Ved flytting eller bare endret filmodus er diffen ellers tom uten forklaring
                let identical = l == r && !matches!(change.status, Status::Added | Status::Deleted);
                (diff, identical.then(|| Notice::info("Innholdet er likt")))
            }
            Err(_) => (
                empty_diff(),
                Some(Notice::warning("Filen er for stor til å vises")),
            ),
        },
    }
}

pub fn empty_diff() -> FileDiff {
    FileDiff::new("", "", ComparisonPolicy::Default).expect("tom diff er aldri for stor")
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

    fn notice(change: &FileChange, content: Result<[String; 2], String>) -> Option<Notice> {
        build_diff(change, content).1
    }

    #[test]
    fn beskjed_når_diffen_ikke_kan_vises() {
        let m = change("a.rs", Status::Modified);
        let text = |l: &str, r: &str| Ok([l.to_owned(), r.to_owned()]);
        assert_eq!(notice(&m, text("a\n", "b\n")), None);
        assert_eq!(
            notice(&m, text("a\n", "a\n")),
            Some(Notice::info("Innholdet er likt"))
        );
        assert_eq!(
            notice(&m, text("a\0", "b")),
            Some(Notice::warning("Binærfil kan ikke vises"))
        );
        assert_eq!(
            notice(&m, Err("borte".into())),
            Some(Notice::warning("borte"))
        );
        // En tom ny fil er ikke «lik»
        assert_eq!(notice(&change("a.rs", Status::Added), text("", "")), None);
    }
}
