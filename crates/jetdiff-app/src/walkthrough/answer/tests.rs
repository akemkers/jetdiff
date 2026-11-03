use jetdiff_files::{ChangedFile, FileStatus};
use serde_json::json;

use super::*;

fn changed_file(path: &str) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        renamed_from: None,
        status: FileStatus::Modified,
    }
}

/// `a.rs` with hunks h1 and h2, `b.rs` with h3, and a lock file.
fn digest() -> Digest {
    let lines = |changed: &[usize]| {
        (0..20)
            .map(|i| {
                if changed.contains(&i) {
                    format!("x{i}\n")
                } else {
                    format!("{i}\n")
                }
            })
            .collect::<String>()
    };
    Digest::new(vec![
        (changed_file("a.rs"), Ok([lines(&[]), lines(&[2, 15])])),
        (changed_file("b.rs"), Ok([lines(&[]), lines(&[5])])),
        (changed_file("Cargo.lock"), Ok(["a\n".into(), "b\n".into()])),
    ])
}

fn output(walkthrough: serde_json::Value) -> String {
    json!({ "is_error": false, "structured_output": walkthrough, "total_cost_usd": 0.002 })
        .to_string()
}

fn step(title: &str, hunks: &[&str]) -> serde_json::Value {
    json!({ "title": title, "hunks": hunks, "weight": "normal", "points": [format!("Why {title}.")], "notes": [] })
}

#[test]
fn steps_point_at_the_hunks_in_the_digest() {
    let answer = output(json!({
        "title": "Retry",
        "summary": "Uploads are retried.",
        "before_you_review": ["The queue lives in memory.", " "],
        "parts": [{
            "title": "Core",
            "blurb": "",
            "steps": [
                { "title": "Queue", "hunks": ["h3", "h1"], "weight": "key", "points": [" A queue. ", "", "In memory."],
                  "notes": [{ "kind": "decision", "text": "Kept in memory." }, { "kind": "opinion", "text": "Nice." }] },
                step("Made up", &["h9"]),
                step("Again", &["h1", "h2", "h2"]),
            ],
        }, { "title": "Empty", "blurb": "", "steps": [] }],
        "not_covered": [],
    }));
    let walkthrough = Walkthrough::from_answer(&answer, &digest()).unwrap();
    assert_eq!(walkthrough.title, "Retry");
    assert_eq!(
        walkthrough.before_you_review,
        ["The queue lives in memory."]
    );
    assert_eq!(walkthrough.parts.len(), 1);
    let steps = &walkthrough.parts[0].steps;
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].weight, Weight::Key);
    assert_eq!(steps[0].points, ["A queue.", "In memory."]);
    assert_eq!(
        steps[0].hunks,
        [HunkRef { file: 1, hunk: 0 }, HunkRef { file: 0, hunk: 0 }]
    );
    assert_eq!(
        steps[0].notes,
        [Note {
            kind: NoteKind::Decision,
            text: "Kept in memory.".into()
        }]
    );
    // h1 was taken by the first step, and h2 counts once
    assert_eq!(steps[1].hunks, [HunkRef { file: 0, hunk: 1 }]);
}

#[test]
fn not_covered_lists_every_file_with_hunks_left() {
    let answer = output(json!({
        "title": "T", "summary": "S",
        "parts": [{ "title": "P", "blurb": "", "steps": [step("One", &["h1"])] }],
        "not_covered": [{ "path": "a.rs", "note": "A comment" }, { "path": "nope.rs", "note": "?" }],
    }));
    let walkthrough = Walkthrough::from_answer(&answer, &digest()).unwrap();
    let not_covered: Vec<_> = walkthrough
        .not_covered
        .iter()
        .map(|file| (file.file, file.hunks.len(), file.note.as_str()))
        .collect();
    assert_eq!(
        not_covered,
        [
            (0, 1, "A comment"),
            (1, 1, ""),
            (2, 0, "Generated file, not sent to the model"),
        ]
    );
}

#[test]
fn answers_that_give_no_walkthrough() {
    let no_steps = output(json!({
        "title": "T", "summary": "S",
        "parts": [{ "title": "P", "blurb": "", "steps": [step("Made up", &["h7"])] }],
        "not_covered": [],
    }));
    let error = |answer: &str| Walkthrough::from_answer(answer, &digest()).unwrap_err();
    assert_eq!(
        error(&no_steps),
        "The model gave no steps that point at the diff"
    );
    let failed = json!({ "is_error": true, "result": "Not logged in" }).to_string();
    assert_eq!(error(&failed), "Not logged in");
    assert_eq!(error("not json"), "Couldn't read the model's answer");
    // Without structured output, the walkthrough is the text of the result
    let as_text = json!({
        "is_error": false,
        "result": json!({ "title": "T", "summary": "S", "parts": [
            { "title": "P", "blurb": "", "steps": [step("One", &["h1"])] }
        ], "not_covered": [] }).to_string(),
    })
    .to_string();
    assert!(Walkthrough::from_answer(&as_text, &digest()).is_ok());
}
