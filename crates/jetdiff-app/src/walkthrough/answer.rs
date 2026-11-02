//! The model's answer as a walkthrough. The model is trusted with grouping and prose only: every
//! alias it gives is looked up in the digest, and one that isn't there is dropped rather than drawn
//! as a step that points at nothing.

use std::collections::HashSet;

use serde::Deserialize;

use super::digest::{Digest, FileContent, HunkRef};
use super::prompt::{MAX_BEFORE_YOU_REVIEW, MAX_NOTES, MAX_PARTS, MAX_STEPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// What the rest depends on, or where the main behaviour lives.
    Key,
    Normal,
    /// Safe to skim.
    Skim,
}

/// Something the reader takes on by accepting the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Choice,
    Upkeep,
    Gap,
    Divergence,
    Untested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub kind: NoteKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub title: String,
    pub text: String,
    pub weight: Weight,
    pub notes: Vec<Note>,
    /// In the order the model gave them.
    pub hunks: Vec<HunkRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub title: String,
    pub blurb: String,
    pub steps: Vec<Step>,
}

/// A file with hunks no step covers, or that wasn't sent or read.
#[derive(Debug, Clone, PartialEq)]
pub struct NotCovered {
    pub file: usize,
    pub hunks: Vec<usize>,
    /// What changed there, from the model, or why it wasn't read.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Walkthrough {
    pub title: String,
    pub summary: String,
    /// What a reviewer needs to know before reading the steps.
    pub before_you_review: Vec<String>,
    pub parts: Vec<Part>,
    pub not_covered: Vec<NotCovered>,
    /// In dollars, when the model said.
    pub cost: Option<f64>,
}

/// What `claude -p --output-format json` writes.
#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: String,
    structured_output: Option<RawWalkthrough>,
    total_cost_usd: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawWalkthrough {
    title: String,
    summary: String,
    before_you_review: Vec<String>,
    parts: Vec<RawPart>,
    skipped: Vec<RawSkipped>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPart {
    title: String,
    blurb: String,
    steps: Vec<RawStep>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawStep {
    title: String,
    hunks: Vec<String>,
    weight: String,
    text: String,
    notes: Vec<RawNote>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawNote {
    kind: String,
    text: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSkipped {
    path: String,
    note: String,
}

impl Walkthrough {
    /// `answer` is what the model wrote, `digest` what it was given.
    pub fn from_answer(answer: &str, digest: &Digest) -> Result<Self, String> {
        let unreadable = || "Couldn't read the model's answer".to_owned();
        let output: Output = serde_json::from_str(answer).map_err(|_| unreadable())?;
        if output.is_error {
            return Err(output.result);
        }
        let raw = match output.structured_output {
            Some(raw) => raw,
            None => serde_json::from_str(&output.result).map_err(|_| unreadable())?,
        };
        let walkthrough = Self::normalize(raw, digest, output.total_cost_usd);
        if walkthrough.parts.is_empty() {
            return Err("The model gave no steps that point at the change".into());
        }
        Ok(walkthrough)
    }

    fn normalize(raw: RawWalkthrough, digest: &Digest, cost: Option<f64>) -> Self {
        let mut used = HashSet::new();
        let (mut steps, mut notes) = (0, 0);
        let mut parts = Vec::new();
        for raw_part in raw.parts.into_iter().take(MAX_PARTS) {
            let mut part_steps = Vec::new();
            for raw_step in raw_part.steps {
                let hunks: Vec<HunkRef> = raw_step
                    .hunks
                    .iter()
                    .filter_map(|alias| digest.hunk(alias.trim()))
                    .filter(|&hunk| used.insert(hunk))
                    .collect();
                if hunks.is_empty() || steps == MAX_STEPS {
                    continue;
                }
                steps += 1;
                let step_notes: Vec<Note> = raw_step
                    .notes
                    .into_iter()
                    .filter_map(note)
                    .take(MAX_NOTES - notes)
                    .collect();
                notes += step_notes.len();
                part_steps.push(Step {
                    title: raw_step.title.trim().to_owned(),
                    text: raw_step.text.trim().to_owned(),
                    weight: weight(&raw_step.weight),
                    notes: step_notes,
                    hunks,
                });
            }
            if !part_steps.is_empty() {
                parts.push(Part {
                    title: raw_part.title.trim().to_owned(),
                    blurb: raw_part.blurb.trim().to_owned(),
                    steps: part_steps,
                });
            }
        }
        Self {
            title: raw.title.trim().to_owned(),
            summary: raw.summary.trim().to_owned(),
            before_you_review: raw
                .before_you_review
                .iter()
                .map(|point| point.trim().to_owned())
                .filter(|point| !point.is_empty())
                .take(MAX_BEFORE_YOU_REVIEW)
                .collect(),
            not_covered: not_covered(digest, &used, &raw.skipped),
            parts,
            cost,
        }
    }
}

fn weight(weight: &str) -> Weight {
    match weight {
        "key" => Weight::Key,
        "skim" => Weight::Skim,
        _ => Weight::Normal,
    }
}

/// A note of a kind the schema doesn't have is dropped.
fn note(raw: RawNote) -> Option<Note> {
    let kind = match raw.kind.as_str() {
        "choice" => NoteKind::Choice,
        "upkeep" => NoteKind::Upkeep,
        "gap" => NoteKind::Gap,
        "divergence" => NoteKind::Divergence,
        "untested" => NoteKind::Untested,
        _ => return None,
    };
    let text = raw.text.trim().to_owned();
    (!text.is_empty()).then_some(Note { kind, text })
}

fn not_covered(
    digest: &Digest,
    used: &HashSet<HunkRef>,
    skipped: &[RawSkipped],
) -> Vec<NotCovered> {
    let model_note = |path: &str| {
        let skipped = skipped.iter().find(|skipped| skipped.path.trim() == path);
        skipped.map_or(String::new(), |skipped| skipped.note.trim().to_owned())
    };
    let mut not_covered = Vec::new();
    for (file, digest_file) in digest.files().iter().enumerate() {
        let (hunks, note) = match &digest_file.content {
            FileContent::Generated => (
                Vec::new(),
                "Generated file, not sent to the model".to_owned(),
            ),
            FileContent::NotRead(reason) => (Vec::new(), reason.clone()),
            FileContent::Diff(diff) => {
                let left: Vec<usize> = (0..diff.hunks().len())
                    .filter(|&hunk| !used.contains(&HunkRef { file, hunk }))
                    .collect();
                if left.is_empty() {
                    continue;
                }
                (left, model_note(&digest_file.file.path))
            }
        };
        not_covered.push(NotCovered { file, hunks, note });
    }
    not_covered
}

#[cfg(test)]
mod tests;
