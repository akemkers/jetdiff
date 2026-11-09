//! Split or Unified for the same file. What the UI does the same way in all diff modes goes through
//! here, so it doesn't need to know which one is shown.

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

#[derive(Debug, Clone)]
pub enum DiffMode {
    /// Split where each side scrolls on its own, held together by sync scroll.
    Split(DiffState),
    AlignedSplit(UnifiedState),
    Unified(UnifiedState),
}

impl DiffMode {
    /// `fold_unchanged` closes the folds from the start, like `f`.
    pub fn new(
        diff: Rc<FileDiff>,
        unified: bool,
        fold_unchanged: bool,
        split_rows: SplitRows,
    ) -> Self {
        if unified || !diff.can_split() {
            return Self::Unified(rows(diff, RowLayout::Unified, fold_unchanged));
        }
        match split_rows {
            SplitRows::Synced => {
                let folding = split_folding(&diff, fold_unchanged);
                Self::Split(DiffState::new(diff, folding))
            }
            SplitRows::Aligned => Self::AlignedSplit(rows(diff, RowLayout::Split, fold_unchanged)),
        }
    }

    /// Switches diff mode with the cursor on the same line and height. The folding is made again
    /// for the new diff mode, with the folds as `fold_unchanged` says. Unified switches to the
    /// Split that `split_rows` says.
    pub fn toggle(&mut self, fold_unchanged: bool, split_rows: SplitRows, viewport: Viewport) {
        *self = match self {
            Self::Split(state) => {
                Self::Unified(UnifiedState::from_split(state, fold_unchanged, viewport))
            }
            Self::AlignedSplit(state) => {
                Self::Unified(state.realign(RowLayout::Unified, fold_unchanged, viewport))
            }
            Self::Unified(state) => match split_rows {
                SplitRows::Synced => Self::Split(state.to_split(fold_unchanged, viewport)),
                SplitRows::Aligned => {
                    Self::AlignedSplit(state.realign(RowLayout::Split, fold_unchanged, viewport))
                }
            },
        };
    }

    pub fn any_fold_closed(&self) -> bool {
        match self {
            Self::Split(state) => state.folding.any_closed(),
            Self::AlignedSplit(state) | Self::Unified(state) => state.folding.any_closed(),
        }
    }

    pub fn hovered_fold(&self) -> Option<FoldId> {
        match self {
            Self::Split(state) => state.folding.hovered(),
            Self::AlignedSplit(state) | Self::Unified(state) => state.folding.hovered(),
        }
    }

    /// `true` if the hovered fold changed.
    pub fn set_hovered_fold(&mut self, id: Option<FoldId>) -> bool {
        if self.hovered_fold() == id {
            return false;
        }
        match self {
            Self::Split(state) => Rc::make_mut(&mut state.folding).set_hovered(id),
            Self::AlignedSplit(state) | Self::Unified(state) => {
                Rc::make_mut(&mut state.folding).set_hovered(id)
            }
        }
    }

    /// `None` also when the target doesn't fit the diff mode.
    pub fn fold_at(&self, target: PointerTarget) -> Option<FoldId> {
        match (target, self) {
            (PointerTarget::Line(side, line), Self::Split(state)) => {
                state.folding.fold_at_line(side, line)
            }
            (PointerTarget::Row(row), Self::AlignedSplit(state) | Self::Unified(state)) => {
                state.folding.fold_at_row(row)
            }
            _ => None,
        }
    }

    /// Moves the cursor there without scrolling. A target that doesn't fit the diff mode is ignored.
    pub fn click(&mut self, target: PointerTarget) {
        match (target, self) {
            (PointerTarget::Line(side, line), Self::Split(state)) => state.click(side, line),
            (PointerTarget::Row(row), Self::AlignedSplit(state) | Self::Unified(state)) => {
                state.click(row)
            }
            _ => {}
        }
    }

    /// Split scrolls `side`, or the side the cursor is on when it is `None`.
    pub fn wheel(&mut self, side: Option<Side>, delta_y: f32, viewport: Viewport) {
        match self {
            Self::Split(state) => {
                let side = side.unwrap_or(state.master);
                state.wheel(side, delta_y, viewport);
            }
            Self::AlignedSplit(state) | Self::Unified(state) => {
                state.wheel(delta_y, viewport);
            }
        }
    }

    pub fn is_unified(&self) -> bool {
        matches!(self, Self::Unified(_))
    }

    pub fn diff(&self) -> &Rc<FileDiff> {
        match self {
            Self::Split(state) => &state.diff,
            Self::AlignedSplit(state) | Self::Unified(state) => &state.diff.diff,
        }
    }

    /// The target for scroll. The UI animates there.
    pub fn scroll(&self) -> ScrollState {
        match self {
            Self::Split(state) => state.scroll,
            Self::AlignedSplit(state) | Self::Unified(state) => state.scroll,
        }
    }

    pub fn scroll_mut(&mut self) -> &mut ScrollState {
        match self {
            Self::Split(state) => &mut state.scroll,
            Self::AlignedSplit(state) | Self::Unified(state) => &mut state.scroll,
        }
    }

    pub fn init(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.init(viewport),
            Self::AlignedSplit(state) | Self::Unified(state) => state.init(viewport),
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
            Self::AlignedSplit(state) | Self::Unified(state) => {
                state.execute(command, viewport, horizontal)
            }
        }
    }

    pub fn resize(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.resize(viewport),
            Self::AlignedSplit(state) | Self::Unified(state) => state.resize(viewport),
        }
    }

    pub fn rescale(&mut self, previous_line_height: f32, viewport: Viewport, x_scale: f32) {
        match self {
            Self::Split(state) => state.rescale(previous_line_height, viewport, x_scale),
            Self::AlignedSplit(state) | Self::Unified(state) => {
                state.rescale(previous_line_height, viewport, x_scale)
            }
        }
    }

    pub fn land_on_hunk(&mut self, hunk: usize, viewport: Viewport) -> Outcome {
        match self {
            Self::Split(state) => state.land_on_hunk(hunk, viewport),
            Self::AlignedSplit(state) | Self::Unified(state) => state.land_on_hunk(hunk, viewport),
        }
    }

    /// The cursor's line in the newest version, for `y`.
    pub fn cursor_location(&self) -> Cursor {
        match self {
            Self::Split(state) => state.cursor_location(),
            Self::AlignedSplit(state) | Self::Unified(state) => state.cursor_location(),
        }
    }

    /// The hunk the cursor is in, for rollback.
    pub fn cursor_hunk(&self) -> Option<usize> {
        match self {
            Self::Split(state) => state.cursor_hunk(),
            Self::AlignedSplit(state) | Self::Unified(state) => state.cursor_hunk(),
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
        split_rows: SplitRows,
        viewport: Viewport,
    ) {
        let unified = unified || !diff.can_split();
        if unified && !self.is_unified() {
            self.toggle(fold_unchanged, split_rows, viewport);
        }
        match self {
            Self::Split(state) => {
                let folding = split_folding(&diff, fold_unchanged);
                state.replace_diff(diff, folding, viewport);
            }
            Self::AlignedSplit(state) | Self::Unified(state) => {
                state.replace_diff(diff, fold_unchanged, viewport)
            }
        }
        if !unified && self.is_unified() {
            self.toggle(fold_unchanged, split_rows, viewport);
        }
    }
}

fn rows(diff: Rc<FileDiff>, row_layout: RowLayout, fold_unchanged: bool) -> UnifiedState {
    UnifiedState::new(
        Rc::new(UnifiedDiff::with_layout(diff, row_layout)),
        fold_unchanged,
    )
}
