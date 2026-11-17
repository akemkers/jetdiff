//! Split or Unified for the same file. Split is Aligned Split in the terminal, and with sync scroll
//! in the window. What the UI does the same way in all diff modes goes through here, so it doesn't
//! need to know which one is shown.

mod pointer_target;
mod split_rows;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::{FoldId, split_folding};
use crate::layout::Viewport;
use crate::model::FileDiff;
use crate::scroll::ScrollState;
use crate::state::{DiffState, Horizontal, Outcome};
use crate::unified::{RowLayout, UnifiedDiff, UnifiedState};

pub use pointer_target::PointerTarget;
pub use split_rows::SplitRows;

/// The diff mode for one file, and the state for it.
#[derive(Debug, Clone)]
pub struct DiffMode {
    /// Set by the UI when it is made, so Unified knows which Split to switch back to.
    split_rows: SplitRows,
    state: ModeState,
}

#[derive(Debug, Clone)]
pub enum ModeState {
    /// Split where each side scrolls on its own, held together by sync scroll.
    Split(DiffState),
    /// Unified or Aligned Split, as `row_layout()` says.
    Rows(UnifiedState),
}

impl DiffMode {
    /// `fold_unchanged` closes the folds from the start, like `f`.
    pub fn new(
        diff: Rc<FileDiff>,
        unified: bool,
        fold_unchanged: bool,
        split_rows: SplitRows,
    ) -> Self {
        let state = if unified || !diff.can_split() {
            ModeState::Rows(rows(diff, RowLayout::Unified, fold_unchanged))
        } else {
            match split_rows {
                SplitRows::Synced => {
                    let folding = split_folding(&diff, fold_unchanged);
                    ModeState::Split(DiffState::new(diff, folding))
                }
                SplitRows::Aligned => {
                    ModeState::Rows(rows(diff, RowLayout::AlignedSplit, fold_unchanged))
                }
            }
        };
        Self { split_rows, state }
    }

    /// What the UI draws from.
    pub fn state(&self) -> &ModeState {
        &self.state
    }

    /// Switches diff mode with the cursor on the same line and height. The folding is made again
    /// for the new diff mode, with the folds as `fold_unchanged` says.
    pub fn toggle(&mut self, fold_unchanged: bool, viewport: Viewport) {
        self.state = match &self.state {
            ModeState::Split(state) => {
                ModeState::Rows(UnifiedState::from_split(state, fold_unchanged, viewport))
            }
            ModeState::Rows(state) => match (state.diff.row_layout(), self.split_rows) {
                (RowLayout::AlignedSplit, _) => {
                    ModeState::Rows(state.realign(RowLayout::Unified, fold_unchanged, viewport))
                }
                (RowLayout::Unified, SplitRows::Synced) => {
                    ModeState::Split(state.to_split(fold_unchanged, viewport))
                }
                (RowLayout::Unified, SplitRows::Aligned) => ModeState::Rows(state.realign(
                    RowLayout::AlignedSplit,
                    fold_unchanged,
                    viewport,
                )),
            },
        };
    }

    pub fn any_fold_closed(&self) -> bool {
        match &self.state {
            ModeState::Split(state) => state.folding.any_closed(),
            ModeState::Rows(state) => state.folding.any_closed(),
        }
    }

    pub fn hovered_fold(&self) -> Option<FoldId> {
        match &self.state {
            ModeState::Split(state) => state.folding.hovered(),
            ModeState::Rows(state) => state.folding.hovered(),
        }
    }

    /// `true` if the hovered fold changed.
    pub fn set_hovered_fold(&mut self, id: Option<FoldId>) -> bool {
        if self.hovered_fold() == id {
            return false;
        }
        match &mut self.state {
            ModeState::Split(state) => Rc::make_mut(&mut state.folding).set_hovered(id),
            ModeState::Rows(state) => Rc::make_mut(&mut state.folding).set_hovered(id),
        }
    }

    /// `None` also when the target doesn't fit the diff mode.
    pub fn fold_at(&self, target: PointerTarget) -> Option<FoldId> {
        match (target, &self.state) {
            (PointerTarget::Line(side, line), ModeState::Split(state)) => {
                state.folding.fold_at_line(side, line)
            }
            (PointerTarget::Row(row), ModeState::Rows(state)) => state.folding.fold_at_row(row),
            _ => None,
        }
    }

    /// Moves the cursor there without scrolling. A target that doesn't fit the diff mode is ignored.
    pub fn click(&mut self, target: PointerTarget) {
        match (target, &mut self.state) {
            (PointerTarget::Line(side, line), ModeState::Split(state)) => state.click(side, line),
            (PointerTarget::Row(row), ModeState::Rows(state)) => state.click(row),
            _ => {}
        }
    }

    /// Split scrolls `side`, or the side the cursor is on when it is `None`.
    pub fn wheel(&mut self, side: Option<Side>, delta_y: f32, viewport: Viewport) {
        match &mut self.state {
            ModeState::Split(state) => {
                let side = side.unwrap_or(state.master);
                state.wheel(side, delta_y, viewport);
            }
            ModeState::Rows(state) => {
                state.wheel(delta_y, viewport);
            }
        }
    }

    pub fn is_unified(&self) -> bool {
        match &self.state {
            ModeState::Split(_) => false,
            ModeState::Rows(state) => state.diff.row_layout() == RowLayout::Unified,
        }
    }

    pub fn diff(&self) -> &Rc<FileDiff> {
        match &self.state {
            ModeState::Split(state) => &state.diff,
            ModeState::Rows(state) => &state.diff.diff,
        }
    }

    /// The target for scroll. The UI animates there.
    pub fn scroll(&self) -> ScrollState {
        match &self.state {
            ModeState::Split(state) => state.scroll,
            ModeState::Rows(state) => state.scroll,
        }
    }

    pub fn scroll_mut(&mut self) -> &mut ScrollState {
        match &mut self.state {
            ModeState::Split(state) => &mut state.scroll,
            ModeState::Rows(state) => &mut state.scroll,
        }
    }

    pub fn init(&mut self, viewport: Viewport) {
        match &mut self.state {
            ModeState::Split(state) => state.init(viewport),
            ModeState::Rows(state) => state.init(viewport),
        }
    }

    pub fn execute(
        &mut self,
        command: Command,
        viewport: Viewport,
        horizontal: Horizontal,
    ) -> Outcome {
        match &mut self.state {
            ModeState::Split(state) => state.execute(command, viewport, horizontal),
            ModeState::Rows(state) => state.execute(command, viewport, horizontal),
        }
    }

    pub fn resize(&mut self, viewport: Viewport) {
        match &mut self.state {
            ModeState::Split(state) => state.resize(viewport),
            ModeState::Rows(state) => state.resize(viewport),
        }
    }

    pub fn rescale(&mut self, previous_line_height: f32, viewport: Viewport, x_scale: f32) {
        match &mut self.state {
            ModeState::Split(state) => state.rescale(previous_line_height, viewport, x_scale),
            ModeState::Rows(state) => state.rescale(previous_line_height, viewport, x_scale),
        }
    }

    pub fn land_on_hunk(&mut self, hunk: usize, viewport: Viewport) -> Outcome {
        match &mut self.state {
            ModeState::Split(state) => state.land_on_hunk(hunk, viewport),
            ModeState::Rows(state) => state.land_on_hunk(hunk, viewport),
        }
    }

    /// The cursor's line in the newest version, for `y`.
    pub fn cursor_location(&self) -> Cursor {
        match &self.state {
            ModeState::Split(state) => state.cursor_location(),
            ModeState::Rows(state) => state.cursor_location(),
        }
    }

    /// The hunk the cursor is in, for rollback.
    pub fn cursor_hunk(&self) -> Option<usize> {
        match &self.state {
            ModeState::Split(state) => state.cursor_hunk(),
            ModeState::Rows(state) => state.cursor_hunk(),
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
        match &mut self.state {
            ModeState::Split(state) => {
                let folding = split_folding(&diff, fold_unchanged);
                state.replace_diff(diff, folding, viewport);
            }
            ModeState::Rows(state) => state.replace_diff(diff, fold_unchanged, viewport),
        }
        if !unified && self.is_unified() {
            self.toggle(fold_unchanged, viewport);
        }
    }
}

fn rows(diff: Rc<FileDiff>, row_layout: RowLayout, fold_unchanged: bool) -> UnifiedState {
    UnifiedState::new(
        Rc::new(UnifiedDiff::with_layout(diff, row_layout)),
        fold_unchanged,
    )
}
