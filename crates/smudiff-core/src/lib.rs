pub mod by_char;
pub mod by_line;
pub mod by_word;
mod change_corrector;
mod character_utils;
mod chunk_optimizer;
pub mod compare;
pub mod fragments;
pub mod iterable;
pub mod lcs;
mod line_fragment_splitter;
pub mod policy;
pub mod side;
pub mod trim;

pub use compare::{compare_for_viewer, compare_lines, compare_lines_inner, process_blocks};
pub use fragments::{DiffFragment, LineFragment, LineOffsets};
pub use lcs::DiffTooBig;
pub use policy::ComparisonPolicy;
