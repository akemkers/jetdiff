mod by_char;
mod by_line;
mod by_word;
mod change_corrector;
mod character_utils;
mod chunk_optimizer;
mod compare;
mod fragments;
mod iterable;
mod lcs;
mod line_fragment_splitter;
mod policy;
pub mod side;
mod trim;

pub use compare::{compare_for_viewer, compare_lines, compare_lines_inner};
pub use fragments::{DiffFragment, LineFragment, LineOffsets};
pub use lcs::DiffTooBig;
pub use policy::ComparisonPolicy;
