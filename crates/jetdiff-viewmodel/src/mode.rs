//! Split or Unified for the same file. What the UI does the same way in both diff modes goes through
//! here, so it doesn't need to know which one is shown.

use std::rc::Rc;

use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::{FoldId, split_folding};
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
    /// `fold_unchanged` closes the folds from the start, like `f`.
    pub fn new(diff: Rc<FileDiff>, unified: bool, fold_unchanged: bool) -> Self {
        if unified || !diff.can_split() {
            Self::Unified(UnifiedState::new(
                Rc::new(UnifiedDiff::new(diff)),
                fold_unchanged,
            ))
        } else {
            Self::Split(DiffState::new(
                diff.clone(),
                split_folding(&diff, fold_unchanged),
            ))
        }
    }

    /// Switches diff mode with the cursor on the same line and height. The folding is made again
    /// for the new diff mode, with the folds as `fold_unchanged` says.
    pub fn toggle(&mut self, fold_unchanged: bool, viewport: Viewport) {
        *self = match self {
            Self::Split(state) => {
                Self::Unified(UnifiedState::from_split(state, fold_unchanged, viewport))
            }
            Self::Unified(state) => Self::Split(state.to_split(fold_unchanged, viewport)),
        };
    }

    pub fn any_fold_closed(&self) -> bool {
        match self {
            Self::Split(state) => state.folding.any_closed(),
            Self::Unified(state) => state.folding.any_closed(),
        }
    }

    pub fn hovered_fold(&self) -> Option<FoldId> {
        match self {
            Self::Split(state) => state.folding.hovered(),
            Self::Unified(state) => state.folding.hovered(),
        }
    }

    /// `true` if the hovered fold changed.
    pub fn set_hovered_fold(&mut self, id: Option<FoldId>) -> bool {
        if self.hovered_fold() == id {
            return false;
        }
        match self {
            Self::Split(state) => Rc::make_mut(&mut state.folding).set_hovered(id),
            Self::Unified(state) => Rc::make_mut(&mut state.folding).set_hovered(id),
        }
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
        unified: bool,
        fold_unchanged: bool,
        viewport: Viewport,
    ) {
        let unified = unified || !diff.can_split();
        if unified && !self.is_unified() {
            self.toggle(fold_unchanged, viewport);
        }
        match self {
            Self::Split(state) => {
                let folding = split_folding(&diff, fold_unchanged);
                state.replace_diff(diff, folding, viewport);
            }
            Self::Unified(state) => state.replace_diff(diff, fold_unchanged, viewport),
        }
        if !unified && self.is_unified() {
            self.toggle(fold_unchanged, viewport);
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

    fn diff(left_side: &str, right_side: &str) -> Rc<FileDiff> {
        Rc::new(FileDiff::new(left_side, right_side, ComparisonPolicy::Default).unwrap())
    }

    #[test]
    fn switch_back_and_forth() {
        let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let right_side = left_side.replace("a50\n", "x\n");
        let mut mode = DiffMode::new(diff(&left_side, &right_side), false, false);
        mode.init(VIEWPORT);
        let before = mode.cursor_location();

        mode.toggle(false, VIEWPORT);
        assert!(mode.is_unified());
        assert_eq!(mode.cursor_location(), before);
        assert_eq!(mode.cursor_hunk(), Some(0));

        mode.toggle(false, VIEWPORT);
        assert!(!mode.is_unified());
        assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 50));
    }

    #[test]
    fn switch_closes_the_folds_opened_with_o() {
        // One hunk on line 50 of 100, so the folds are 0..46 and 55..101
        let left_side: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let right_side = left_side.replace("a50\n", "x\n");
        let mut mode = DiffMode::new(diff(&left_side, &right_side), false, true);
        mode.init(VIEWPORT);
        let mut run = |command| mode.execute(command, VIEWPORT, Horizontal::default());
        run(Command::GoToBottom);
        run(Command::FoldOpen);
        run(Command::GoToBottom);
        run(Command::MoveUp);
        assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 99));

        // The line is in a closed fold in Unified, so the cursor moves out to the fold
        mode.toggle(true, VIEWPORT);
        assert!(mode.is_unified());
        assert!(mode.any_fold_closed());
        assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 55));

        mode.toggle(true, VIEWPORT);
        assert_eq!(mode.cursor_location(), Cursor::new(Side::Right, 55));
    }

    #[test]
    fn added_and_deleted_files_are_shown_unified() {
        let mode = DiffMode::new(diff("", "a\n"), false, true);
        assert!(mode.is_unified());

        let changed = diff("a\nb\n", "a\nc\n");
        let mut mode = DiffMode::new(changed.clone(), false, true);
        mode.init(VIEWPORT);
        mode.replace_diff(diff("a\nb\n", ""), false, true, VIEWPORT);
        assert!(mode.is_unified());
        mode.replace_diff(changed, false, true, VIEWPORT);
        assert!(!mode.is_unified());
    }
}
