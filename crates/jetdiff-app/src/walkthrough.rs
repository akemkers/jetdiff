//! A walkthrough of the diff being shown: its hunks grouped into steps in reading order, each
//! explained, written by a model that reads the diff.

pub mod answer;
pub mod cache;
pub mod digest;
pub mod job;
pub mod prompt;
pub mod reading;

pub use answer::{NotCovered, Note, NoteKind, Part, Step, Walkthrough, Weight};
pub use digest::{Digest, DigestFile, FileContent, HunkRef};
pub use job::{Job, Writer};
pub use reading::{ContentsRow, Page, Reading};
