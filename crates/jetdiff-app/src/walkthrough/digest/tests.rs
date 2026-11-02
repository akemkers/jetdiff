use jetdiff_files::FileStatus;

use super::*;

fn changed_file(path: &str, status: FileStatus) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        renamed_from: None,
        status,
    }
}

fn text(left_side: &str, right_side: &str) -> Result<[String; 2], String> {
    Ok([left_side.into(), right_side.into()])
}

#[test]
fn hunks_with_aliases_and_context() {
    let digest = Digest::new(vec![
        (
            changed_file("a.rs", FileStatus::Modified),
            text(
                "1\n2\n3\n4\n5\n6\n7\n8\n9\n",
                "1\n2\n3\nfour\n5\n6\n7\n8\nnine\n",
            ),
        ),
        (changed_file("b.rs", FileStatus::Added), text("", "b\n")),
    ]);
    assert_eq!(
        digest.text(),
        "\
=== a.rs (modified)
[h1] @@ -1,7 +1,7 @@
 1
 2
 3
-4
+four
 5
 6
 7
[h2] @@ -6,5 +6,5 @@
 6
 7
 8
-9
+nine
\x20
=== b.rs (added)
[h3] @@ -0,0 +1,2 @@
+b
+
"
    );
    assert_eq!(digest.hunk_count(), 3);
    assert_eq!(digest.hunk("h2"), Some(HunkRef { file: 0, hunk: 1 }));
    assert_eq!(digest.hunk("h3"), Some(HunkRef { file: 1, hunk: 0 }));
    assert_eq!(digest.hunk("h4"), None);
    assert_eq!(digest.hunk("h0"), None);
    assert_eq!(digest.hunk("x1"), None);
}

#[test]
fn generated_and_unreadable_files_are_listed_but_not_sent() {
    let digest = Digest::new(vec![
        (
            changed_file("Cargo.lock", FileStatus::Modified),
            text("a\n", "b\n"),
        ),
        (
            changed_file("logo.png", FileStatus::Modified),
            text("\0", "\0\0"),
        ),
        (
            changed_file("gone.rs", FileStatus::Modified),
            Err("gone.rs: not found".into()),
        ),
    ]);
    assert_eq!(digest.text(), "");
    assert_eq!(digest.files().len(), 3);
    assert!(matches!(digest.files()[0].content, FileContent::Generated));
    let reasons: Vec<_> = digest.files()[1..]
        .iter()
        .map(|file| match &file.content {
            FileContent::NotRead(reason) => reason.as_str(),
            _ => "read",
        })
        .collect();
    assert_eq!(
        reasons,
        ["Binary file can't be shown", "gone.rs: not found"]
    );
}
