//! What the model is told, and the shape of its answer. A walkthrough orders and explains a change,
//! it doesn't review it: asked for both, a model puts "make sure that" on every step. What the reader
//! takes on by accepting the change is a short list of note kinds instead, and most steps have none.
//! The rules build on herdr-walkthrough by Devrim Mazlum (MIT).

use serde_json::{Value, json};

use super::digest::Digest;

/// Part of the cache key, since another prompt writes another walkthrough.
pub const PROMPT_VERSION: u32 = 3;

pub const MAX_STEPS: usize = 16;
pub const MAX_PARTS: usize = 6;
pub const MAX_NOTES: usize = 6;
pub const MAX_BEFORE_YOU_REVIEW: usize = 4;
/// The contents column is narrow.
pub const MAX_PART_TITLE: usize = 28;
/// About two or three hunks a step.
const HUNKS_PER_STEP: f32 = 2.5;
const STEPS_PER_PART: usize = 3;

const SYSTEM: &str = "\
You guide an engineer through a code change they did not write: from a colleague, a contributor or a coding agent. They are reviewing it. They know the project and what it is for, but not the details of the code this change touches. Help them read it in an order that makes sense, so they understand what it does and what they accept by taking it. You do not review or grade it.

Give three things.

1. An order. A diff lists files by name, which is rarely how a change is best understood. Put the hunks that serve one purpose into one step, across files, and order the steps so each can be followed with only what came before it: types, schemas and entry points before the code that uses them. A test goes in the step with the code it tests, unless the change is mostly tests. Group the steps into a few parts.

2. An explanation. For each step, three to five plain sentences on how the behaviour is different after the change, and why these hunks belong together. Don't explain what the project or the app is. Explain the details instead: the first time a type, function or term from this area comes up, say in a few words what it is for, unless its name says it. Keep sentences short. The reader sees the code under your text, so don't tell them about the edits (\"adds a parameter\", \"renames foo to bar\", \"updates imports\"). Describe, never instruct: no \"make sure\", \"verify\", \"double-check\", \"consider\" or \"be careful\".

3. Notes, only where they earn their place. A note is something the reader takes on by accepting the change, which the explanation alone would let slip by. Each has one kind:
- \"choice\": a decision that could reasonably have gone another way, like a new default for everyone, a platform left out, a new setting, or data kept somewhere new.
- \"upkeep\": something the project keeps up from now on, like a new dependency, a build step, code in another language, or a second copy of existing logic.
- \"gap\": something users still can't do, or will see fail, after the change, as the code or its docs say.
- \"divergence\": two paths to the same thing that now behave differently for users, like two platforms, or an old and a new path.
- \"untested\": what the change mainly promises is exercised by no test.
A note is one or two sentences on a concrete consequence for users or maintainers, resting on the step's own hunks. It is never a suspected bug, a remark on style or naming, a suggestion or an instruction. Most steps have no notes. When in doubt, leave it out.

The summary is two or three sentences on what the change means for the people who use the software: what they can do now, or no longer run into. Work it out from the code: text shown to users, docs, tests and their names, error handling, and what the new code is wired into. When nothing changes for users, say what it changes for the people working on the code. It lists no risks.

before_you_review: two to four short points a reviewer needs before reading the steps, like a concept the change introduces, the one decision the rest follows from, or where the main behaviour lives. Each is one plain sentence. They don't repeat the summary or the steps.

Rules:
- Point at hunks only with the aliases in square brackets in the digest (h1, h2, ...), exactly as written. Never make one up.
- Use each hunk at most once, in the step where it matters most.
- Leave purely mechanical hunks out rather than padding a step with them. Everything left out is still shown to the reader, file by file.
- skipped: for every file with hunks you left out, one entry with the path exactly as in the digest, and one short line on what changed there, so the reader can trust it without opening it: \"Strings for the new dialog\", \"The snapshot follows the renamed field\", \"The same rename as in the CLI\". A file whose hunks are all in steps has no entry.
- weight: \"key\" only for the one or two steps the rest depends on, or where the main behaviour lives. \"skim\" for supporting changes that are safe to skim, like docs, wiring and fixtures. \"normal\" otherwise.
- Titles say in plain words what a step is about (\"Retry queue for failed uploads\"), never the act of reading it (\"Look at the tests\").
- Write plain sentences: no Markdown, no lists, no code fences.

Answer with a single JSON object and nothing else.";

pub fn system(language: &str) -> String {
    format!(
        "{SYSTEM}\n\nWrite all prose in {language}: titles, the summary, part blurbs, explanations, notes and skipped notes. Keep the aliases, the weight and kind values, identifiers, paths and API names exactly as they are."
    )
}

/// `what` says what was read, like "Branch feature against main".
pub fn prompt(digest: &Digest, what: &str) -> String {
    let hunks = digest.hunk_count();
    let steps = ((hunks as f32 / HUNKS_PER_STEP).round() as usize).clamp(1, MAX_STEPS);
    let parts = steps.div_ceil(STEPS_PER_PART).clamp(1, MAX_PARTS);
    let key = if steps <= 3 { 1 } else { 2 };
    let notes = steps.div_ceil(STEPS_PER_PART).clamp(1, MAX_NOTES);
    let files = digest.files().len();
    format!(
        "The change: {what}

It has {files} file(s) and {hunks} hunk(s) to read. Aim for about {steps} step(s) in about {parts} part(s), never more than {MAX_STEPS} steps or {MAX_PARTS} parts. Fewer, fuller steps beat many thin ones.
Mark at most {key} step(s) \"key\". Write at most {notes} note(s) in the whole walkthrough. None is a fine answer.
Part titles show in a narrow column: one to three words, at most {MAX_PART_TITLE} characters.

The digest: each file starts with \"=== path\", each hunk with its alias in square brackets and its @@ header, then its lines.

{}",
        digest.text()
    )
}

pub fn schema() -> Value {
    let text = json!({ "type": "string" });
    let note = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "text"],
        "properties": {
            "kind": { "type": "string", "enum": ["choice", "upkeep", "gap", "divergence", "untested"] },
            "text": text,
        },
    });
    let step = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "hunks", "weight", "text", "notes"],
        "properties": {
            "title": text,
            "hunks": { "type": "array", "items": text },
            "weight": { "type": "string", "enum": ["key", "normal", "skim"] },
            "text": text,
            "notes": { "type": "array", "items": note },
        },
    });
    let part = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "blurb", "steps"],
        "properties": {
            "title": text,
            "blurb": text,
            "steps": { "type": "array", "items": step },
        },
    });
    let skipped = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "note"],
        "properties": { "path": text, "note": text },
    });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "summary", "before_you_review", "parts", "skipped"],
        "properties": {
            "title": text,
            "summary": text,
            "before_you_review": { "type": "array", "items": text },
            "parts": { "type": "array", "items": part },
            "skipped": { "type": "array", "items": skipped },
        },
    })
}
