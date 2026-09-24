//! Inngangen for diff. Tilsvarer `compareLines`, `compareLinesInner` og `processBlocks` i
//! `ComparisonManagerImpl.java`.

use crate::by_line;
use crate::by_word;
use crate::fragments::{DiffFragment, LineFragment, LineOffsets, convert_into_line_fragments};
use crate::lcs::DiffTooBig;
use crate::policy::{ComparisonPolicy, is_equal_texts};

/// `DiffConfig.MAX_BAD_LINES`: etter så mange for store fragmenter gir vi opp ord-diff.
pub const MAX_BAD_LINES: usize = 3;

/// Linjediff uten inner fragments. Linjer deles kun på `'\n'`, en `'\r'` er vanlig innhold.
pub fn compare_lines(
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
) -> Result<Vec<LineFragment>, DiffTooBig> {
    let line_offsets1 = LineOffsets::new(text1);
    let line_offsets2 = LineOffsets::new(text2);
    let lines1 = line_offsets1.line_contents(text1);
    let lines2 = line_offsets2.line_contents(text2);

    let changes = by_line::compare(&lines1, &lines2, policy)?;
    Ok(convert_into_line_fragments(
        &line_offsets1,
        &line_offsets2,
        &changes,
    ))
}

/// Linjediff med ord-fragmenter (`InnerFragmentsPolicy.WORDS`). Bare linjediffen kan gi
/// `DiffTooBig`, for store fragmenter får `inner: None`.
pub fn compare_lines_inner(
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
) -> Result<Vec<LineFragment>, DiffTooBig> {
    let line_fragments = compare_lines(text1, text2, policy)?;
    Ok(create_inner_fragments(
        &line_fragments,
        text1,
        text2,
        policy,
    ))
}

/// Det side-ved-side-viseren viser for BY_WORD: ord-diff, så `process_blocks` med squash.
pub fn compare_for_viewer(
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
) -> Result<Vec<LineFragment>, DiffTooBig> {
    let words = compare_lines_inner(text1, text2, policy)?;
    Ok(process_blocks(&words))
}

fn create_inner_fragments(
    line_fragments: &[LineFragment],
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
) -> Vec<LineFragment> {
    let mut result = Vec::with_capacity(line_fragments.len());

    let mut too_big_chunks_count = 0;
    for fragment in line_fragments {
        assert!(fragment.inner.is_none());

        // Ikke prøv flere etter noen feil
        let try_compute_differences = too_big_chunks_count < MAX_BAD_LINES;
        match create_inner_fragments_for(fragment, text1, text2, policy, try_compute_differences) {
            Ok(fragments) => result.extend(fragments),
            Err(DiffTooBig) => {
                result.push(fragment.clone());
                too_big_chunks_count += 1;
            }
        }
    }

    result
}

fn create_inner_fragments_for(
    fragment: &LineFragment,
    text1: &str,
    text2: &str,
    policy: ComparisonPolicy,
    try_compute_differences: bool,
) -> Result<Vec<LineFragment>, DiffTooBig> {
    let sub_sequence1 = &text1[fragment.offsets1.clone()];
    let sub_sequence2 = &text2[fragment.offsets2.clone()];

    if fragment.lines1.is_empty() || fragment.lines2.is_empty() {
        // Insert eller delete
        if is_equal_texts(sub_sequence1, sub_sequence2, policy) {
            return Ok(vec![fragment.with_inner(Some(Vec::new()))]);
        }
        return Ok(vec![fragment.clone()]);
    }

    if !try_compute_differences {
        return Ok(vec![fragment.clone()]);
    }

    create_inner_word_fragments(fragment, sub_sequence1, sub_sequence2, policy)
}

fn create_inner_word_fragments(
    fragment: &LineFragment,
    sub_sequence1: &str,
    sub_sequence2: &str,
    policy: ComparisonPolicy,
) -> Result<Vec<LineFragment>, DiffTooBig> {
    let line_blocks = by_word::compare_and_split(sub_sequence1, sub_sequence2, policy)?;
    assert!(!line_blocks.is_empty());

    let start_offset1 = fragment.offsets1.start;
    let start_offset2 = fragment.offsets2.start;

    let mut current_start_line1 = fragment.lines1.start;
    let mut current_start_line2 = fragment.lines2.start;

    let last = line_blocks.len() - 1;
    let mut chunks = Vec::with_capacity(line_blocks.len());
    for (i, block) in line_blocks.into_iter().enumerate() {
        let offsets = block.offsets;

        // Siste blokk får fragmentets sluttlinje, ellers blir tom siste linje feil
        let current_end_line1 = if i != last {
            current_start_line1 + block.newlines1
        } else {
            fragment.lines1.end
        };
        let current_end_line2 = if i != last {
            current_start_line2 + block.newlines2
        } else {
            fragment.lines2.end
        };

        chunks.push(LineFragment::new(
            current_start_line1..current_end_line1,
            current_start_line2..current_end_line2,
            offsets.start1 + start_offset1..offsets.end1 + start_offset1,
            offsets.start2 + start_offset2..offsets.end2 + start_offset2,
            Some(block.fragments),
        ));

        current_start_line1 = current_end_line1;
        current_start_line2 = current_end_line2;
    }
    Ok(chunks)
}

/// `processBlocks` med squash: fragmenter som ligger inntil hverandre slås sammen. `trim`
/// hører til "ignore whitespaces and empty lines", som vi ikke har.
fn process_blocks(old_fragments: &[LineFragment]) -> Vec<LineFragment> {
    old_fragments
        .chunk_by(is_adjoining)
        .map(do_squash)
        .collect()
}

fn do_squash(old_fragments: &[LineFragment]) -> LineFragment {
    assert!(!old_fragments.is_empty());
    if old_fragments.len() == 1 {
        return old_fragments[0].clone();
    }

    let first_fragment = &old_fragments[0];
    let last_fragment = &old_fragments[old_fragments.len() - 1];

    let mut new_inner_fragments: Vec<DiffFragment> = Vec::new();
    for fragment in old_fragments {
        let shift1 = fragment.offsets1.start - first_fragment.offsets1.start;
        let shift2 = fragment.offsets2.start - first_fragment.offsets2.start;
        for inner in extract_inner_fragments(fragment) {
            match new_inner_fragments.last_mut() {
                Some(previous) if is_adjoining_inner(previous, &inner, shift1, shift2) => {
                    previous.offsets1.end = inner.offsets1.end + shift1;
                    previous.offsets2.end = inner.offsets2.end + shift2;
                }
                _ => new_inner_fragments.push(DiffFragment {
                    offsets1: inner.offsets1.start + shift1..inner.offsets1.end + shift1,
                    offsets2: inner.offsets2.start + shift2..inner.offsets2.end + shift2,
                }),
            }
        }
    }

    LineFragment::new(
        first_fragment.lines1.start..last_fragment.lines1.end,
        first_fragment.lines2.start..last_fragment.lines2.end,
        first_fragment.offsets1.start..last_fragment.offsets1.end,
        first_fragment.offsets2.start..last_fragment.offsets2.end,
        Some(new_inner_fragments),
    )
}

fn is_adjoining(before: &LineFragment, after: &LineFragment) -> bool {
    before.lines1.end == after.lines1.start
        && before.lines2.end == after.lines2.start
        && before.offsets1.end == after.offsets1.start
        && before.offsets2.end == after.offsets2.start
}

fn is_adjoining_inner(
    before: &DiffFragment,
    after: &DiffFragment,
    shift1: usize,
    shift2: usize,
) -> bool {
    before.offsets1.end == after.offsets1.start + shift1
        && before.offsets2.end == after.offsets2.start + shift2
}

fn extract_inner_fragments(fragment: &LineFragment) -> Vec<DiffFragment> {
    match &fragment.inner {
        Some(inner) => inner.clone(),
        None => vec![DiffFragment {
            offsets1: 0..fragment.offsets1.len(),
            offsets2: 0..fragment.offsets2.len(),
        }],
    }
}
