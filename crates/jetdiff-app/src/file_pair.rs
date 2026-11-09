//! A File pair: two files on disk, read at startup and again on a source reload.

use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Language};

use crate::notification::Notification;

pub struct FilePair {
    /// Left first.
    paths: [String; 2],
}

impl FilePair {
    pub fn new(left_side: String, right_side: String) -> Self {
        Self {
            paths: [left_side, right_side],
        }
    }

    pub fn paths(&self) -> &[String; 2] {
        &self.paths
    }

    /// The new diff, unless the read failed, and the brief that says how it went.
    pub fn source_reload(&self) -> (Option<FileDiff>, Notification) {
        match self.read() {
            Ok(diff) => (Some(diff), Notification::reloaded()),
            Err(e) => (None, Notification::warning(e)),
        }
    }

    pub fn read(&self) -> Result<FileDiff, String> {
        let [left_side, right_side] = &self.paths;
        let (left_side_text, right_side_text) = (read_file(left_side)?, read_file(right_side)?);
        let mut diff = FileDiff::new(&left_side_text, &right_side_text, ComparisonPolicy::Default)
            .map_err(|_| "the files are too big to diff".to_owned())?;
        // Right first; Left can be a temporary file from git difftool
        if let Some(language) =
            Language::from_path(right_side).or_else(|| Language::from_path(left_side))
        {
            diff.highlight_syntax(language);
        }
        Ok(diff)
    }
}

fn read_file(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binary files and wrong encoding are shown with replacement characters instead of failing
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jetdiff_viewmodel::Side;

    use super::*;
    use crate::notification::NotificationLevel;

    /// Removed also when the test fails.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("jetdiff-file-pair-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, name: &str, text: &str) -> String {
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            path.to_str().unwrap().to_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_source_reload_reads_both_files_again() {
        let dir = TempDir::new();
        let file_pair = FilePair::new(dir.write("a.rs", "a\n"), dir.write("b.rs", "b\n"));
        assert_eq!(file_pair.read().unwrap().text(Side::Right), "b\n");

        dir.write("a.rs", "a2\n");
        dir.write("b.rs", "b2\n");
        let (diff, brief) = file_pair.source_reload();
        let diff = diff.unwrap();
        assert_eq!(diff.text(Side::Left), "a2\n");
        assert_eq!(diff.text(Side::Right), "b2\n");
        assert_eq!(brief, Notification::info("Reloaded"));

        std::fs::remove_dir_all(&dir.0).unwrap();
        let (diff, brief) = file_pair.source_reload();
        assert!(diff.is_none());
        assert_eq!(brief.level, NotificationLevel::Warning);
    }
}
