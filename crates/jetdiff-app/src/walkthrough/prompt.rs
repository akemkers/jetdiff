//! What the model is told, and the shape of its answer. The text is in `prompt/`, read in when
//! jetdiff is built and sent as it is written there, with `{name}` filled in.
//!
//! A walkthrough orders and explains a diff, it doesn't review it: asked for both, a model puts
//! "make sure that" on every step. What the reader takes on by accepting the diff is a short list of
//! note kinds instead, and most steps have none. The rules build on herdr-walkthrough by Devrim
//! Mazlum (MIT).

use serde_json::Value;

use super::digest::Digest;

pub const MAX_STEPS: usize = 16;
pub const MAX_PARTS: usize = 6;
pub const MAX_NOTES: usize = 6;
pub const MAX_BEFORE_YOU_REVIEW: usize = 4;
/// The contents column is narrow.
pub const MAX_PART_TITLE: usize = 28;
pub const MAX_STEP_TITLE: usize = 40;
/// About two or three hunks a step.
const HUNKS_PER_STEP: f32 = 2.5;
const STEPS_PER_PART: usize = 3;

const SYSTEM: &str = include_str!("prompt/system.md");
const LANGUAGE: &str = include_str!("prompt/language.md");
const REQUEST: &str = include_str!("prompt/request.md");
const SCHEMA: &str = include_str!("prompt/schema.json");

pub fn system(language: &str) -> String {
    let language = fill(LANGUAGE, &[("language", language.to_owned())]);
    format!("{}\n\n{}", SYSTEM.trim_end(), language.trim_end())
}

/// `what` says what was read, like "Branch feature against main". The digest comes last, after the
/// fields are filled in, since code has braces of its own.
pub fn prompt(digest: &Digest, what: &str) -> String {
    let hunks = digest.hunk_count();
    let steps = ((hunks as f32 / HUNKS_PER_STEP).round() as usize).clamp(1, MAX_STEPS);
    let parts = steps.div_ceil(STEPS_PER_PART).clamp(1, MAX_PARTS);
    let key_steps = if steps <= 3 { 1 } else { 2 };
    let notes = steps.div_ceil(STEPS_PER_PART).clamp(1, MAX_NOTES);
    let request = fill(
        REQUEST,
        &[
            ("files", digest.files().len().to_string()),
            ("hunks", hunks.to_string()),
            ("steps", steps.to_string()),
            ("parts", parts.to_string()),
            ("max_steps", MAX_STEPS.to_string()),
            ("max_parts", MAX_PARTS.to_string()),
            ("key_steps", key_steps.to_string()),
            ("notes", notes.to_string()),
            ("max_part_title", MAX_PART_TITLE.to_string()),
            ("max_step_title", MAX_STEP_TITLE.to_string()),
            // Last, so braces in a commit subject are left as they are
            ("what", what.to_owned()),
        ],
    );
    format!("{}\n\n{}", request.trim_end(), digest.text())
}

pub fn schema() -> Value {
    serde_json::from_str(SCHEMA).expect("prompt/schema.json is valid JSON")
}

/// Each `{name}` in `template` replaced by its value.
fn fill(template: &str, values: &[(&str, String)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

#[cfg(test)]
mod tests {
    use jetdiff_files::{ChangedFile, FileStatus};

    use super::*;

    fn digest() -> Digest {
        let file = ChangedFile {
            path: "a.rs".into(),
            renamed_from: None,
            status: FileStatus::Modified,
        };
        Digest::new(vec![(
            file,
            Ok(["fn a() {}\n".into(), "fn b() {}\n".into()]),
        )])
    }

    #[test]
    fn every_field_in_the_request_is_filled() {
        let prompt = prompt(&digest(), "Branch {feature}");
        let (request, digest) = prompt.split_once("\n\n=== a.rs").unwrap();
        assert!(request.starts_with("The change: Branch {feature}\n"));
        assert!(request.contains("It has 1 file(s) and 1 hunk(s)"));
        let without_what = request.replace("{feature}", "");
        assert!(!without_what.contains('{'), "{without_what}");
        // The code keeps its braces
        assert!(digest.contains("fn b() {}"));
    }

    #[test]
    fn the_language_follows_the_rules() {
        let system = system("Norwegian");
        assert!(system.starts_with("You guide an engineer"));
        assert!(system.ends_with("Write all prose in Norwegian: titles, the summary, part blurbs, points, notes and not covered notes. Keep the aliases, the weight and kind values, identifiers, paths and API names exactly as they are."));
    }

    #[test]
    fn the_schema_is_json() {
        let required = &schema()["required"];
        assert_eq!(
            required,
            &serde_json::json!([
                "title",
                "summary",
                "before_you_review",
                "parts",
                "not_covered"
            ])
        );
    }
}
