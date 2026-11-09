//! What is visible of one code view in a frame, the same in Split and Unified.

use std::ops::RangeInclusive;

use jetdiff_viewmodel::LineLayout;

pub struct Visible {
    /// Logical lines, also those hidden in a closed fold.
    pub logical: RangeInclusive<usize>,
    /// Visual lines, where a closed fold is one line.
    pub visual: RangeInclusive<usize>,
    /// The lines that have text and a number. Closed folds have neither
    /// (`hideLineNumberPredicate`).
    pub lines: Vec<usize>,
}

impl Visible {
    pub fn new(layout: &LineLayout, scroll_y: f32, height: f32) -> Self {
        let (first, last) = layout.painted_lines(scroll_y, height);
        let (first_visual, last_visual) = layout.painted_visual_lines(scroll_y, height);
        let visual = first_visual..=last_visual.min(layout.visual_line_count() - 1);
        let lines = visual
            .clone()
            .filter(|&visual_line| layout.fold_at_visual(visual_line).is_none())
            .map(|visual_line| layout.visual_to_logical(visual_line))
            .collect();
        Self {
            logical: first..=last.min(layout.line_count() - 1),
            visual,
            lines,
        }
    }
}
