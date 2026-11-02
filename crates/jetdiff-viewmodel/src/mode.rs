//! Split or Unified for the same file. What the UI does the same way in both diff modes goes through
//! here, so it doesn't need to know which one is shown.

use std::rc::Rc;

use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::layout::Viewport;
use crate::model::FileDiff;
use crate::scroll::ScrollState;
use crate::state::{DiffState, Horizontal, Outcome};
use crate::unified::{UnifiedDiff, UnifiedState};

#[derive(Debug, Clone)]
pub enum DiffMode {
    Split(DiffState),
    Unified(UnifiedState),
}

impl DiffMode {
    /// `folding` is only used in Split.
    pub fn new(diff: Rc<FileDiff>, folding: Rc<Folding>, unified: bool) -> Self {
        if unified || !diff.can_split() {
            Self::Unified(UnifiedState::new(Rc::new(UnifiedDiff::new(diff))))
        } else {
            Self::Split(DiffState::new(diff, folding))
        }
    }

    /// Switches diff mode with the cursor on the same line and height. `folding` is used when
    /// switching to Split, and is made again because Unified has no folding.
    pub fn toggle(&mut self, folding: Rc<Folding>, viewport: Viewport) {
        *self = match self {
            Self::Split(state) => Self::Unified(UnifiedState::from_split(state, viewport)),
            Self::Unified(state) => Self::Split(state.to_split(folding, viewport)),
        };
    }

    pub fn is_unified(&self) -> bool {
        matches!(self, Self::Unified(_))
    }

    pub fn diff(&self) -> &Rc<FileDiff> {
        match self {
            Self::Split(state) => &state.diff,
            Self::Unified(state) => &state.diff.diff,
        }
    }

    /// The target for scroll. The UI animates there.
    pub fn scroll(&self) -> ScrollState {
        match self {
            Self::Split(state) => state.scroll,
            Self::Unified(state) => state.scroll,
        }
    }

    pub fn scroll_mut(&mut self) -> &mut ScrollState {
        match self {
            Self::Split(state) => &mut state.scroll,
            Self::Unified(state) => &mut state.scroll,
        }
    }

    pub fn init(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.init(viewport),
            Self::Unified(state) => state.init(viewport),
        }
    }

    pub fn execute(
        &mut self,
        command: Command,
        viewport: Viewport,
        horizontal: Horizontal,
    ) -> Outcome {
        match self {
            Self::Split(state) => state.execute(command, viewport, horizontal),
            Self::Unified(state) => state.execute(command, viewport, horizontal),
        }
    }

    pub fn resize(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.resize(viewport),
            Self::Unified(state) => state.resize(viewport),
        }
    }

    pub fn rescale(&mut self, previous_line_height: f32, viewport: Viewport, x_scale: f32) {
        match self {
            Self::Split(state) => state.rescale(previous_line_height, viewport, x_scale),
            Self::Unified(state) => state.rescale(previous_line_height, viewport, x_scale),
        }
    }

    pub fn land_on_hunk(&mut self, hunk: usize, viewport: Viewport) -> Outcome {
        match self {
            Self::Split(state) => state.land_on_hunk(hunk, viewport),
            Self::Unified(state) => state.land_on_hunk(hunk, viewport),
        }
    }

    /// The cursor's line in the newest version, for `y`.
    pub fn cursor_location(&self) -> Cursor {
        match self {
            Self::Split(state) => state.cursor_location(),
            Self::Unified(state) => state.cursor_location(),
        }
    }

    /// The hunk the cursor is in, for rollback.
    pub fn cursor_hunk(&self) -> Option<usize> {
        match self {
            Self::Split(state) => state.cursor_hunk(),
            Self::Unified(state) => state.cursor_hunk(),
        }
    }

    /// The same file with new content. If the file becomes added or deleted, it switches to Unified before
    /// the content is replaced, so the position is kept. If it no longer is, and `unified` is off,
    /// it switches back to Split afterwards.
    pub fn replace_diff(
        &mut self,
        diff: Rc<FileDiff>,
        folding: Rc<Folding>,
        unified: bool,
        viewport: Viewport,
    ) {
        let unified = unified || !diff.can_split();
        if unified && !self.is_unified() {
            self.toggle(folding.clone(), viewport);
        }
        match self {
            Self::Split(state) => state.replace_diff(diff, folding.clone(), viewport),
            Self::Unified(state) => state.replace_diff(diff, viewport),
        }
        if !unified && self.is_unified() {
            self.toggle(folding, viewport);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;
    use jetdiff_core::side::Side;

    const VIEWPORT: Viewport = Viewport {
        height: 200.,
        line_height: 10.,
    };

    #[test]
    fn switch_back_and_forth() {
        let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let right_side = left_side.replace("a50\n", "x\n");
        let diff =
            Rc::new(FileDiff::new(&left_side, &right_side, ComparisonPolicy::Default).unwrap());
        let folding = || Rc::new(Folding::none(&diff));
        let mut mode = DiffMode::new(diff.clone(), folding(), false);
        mode.init(VIEWPORT);
        let before = mode.cursor_location();

        mode.toggle(folding(), VIEWPORT);
        assert!(mode.is_unified());
        assert_eq!(mode.cursor_location(), before);
        assert_eq!(mode.cursor_hunk(), Some(0));

        mode.toggle(folding(), VIEWPORT);
        assert!(!mode.is_unified());
        assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 50));
    }

    #[test]
    fn added_and_deleted_files_are_shown_unified() {
        let diff = |left_side: &str, right_side: &str| {
            Rc::new(FileDiff::new(left_side, right_side, ComparisonPolicy::Default).unwrap())
        };
        let added_file = diff("", "a\n");
        let mode = DiffMode::new(
            added_file.clone(),
            Rc::new(Folding::none(&added_file)),
            false,
        );
        assert!(mode.is_unified());

        let changed = diff("a\nb\n", "a\nc\n");
        let folding = Rc::new(Folding::none(&changed));
        let mut mode = DiffMode::new(changed.clone(), folding.clone(), false);
        mode.init(VIEWPORT);
        let deleted = diff("a\nb\n", "");
        mode.replace_diff(deleted, folding.clone(), false, VIEWPORT);
        assert!(mode.is_unified());
        mode.replace_diff(changed, folding, false, VIEWPORT);
        assert!(!mode.is_unified());
    }
}
