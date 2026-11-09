//! `EditorScrollingPositionKeeper` for one code view: keeps the cursor, or the top line if the
//! cursor is outside, at the same place on screen through a fold change.

use crate::layout::{LineLayout, Viewport};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CodeViewKeeper {
    /// Logical line and distance from the top of the viewport.
    anchor: (usize, f32),
    keeps_cursor: bool,
}

impl CodeViewKeeper {
    pub(crate) fn save(
        layout: &LineLayout,
        top: f32,
        cursor_line: usize,
        viewport: Viewport,
    ) -> Self {
        let cursor_y = layout.line_top(cursor_line);
        let off_screen =
            cursor_y + layout.line_height() <= top || cursor_y >= top + viewport.height;
        let anchor = if off_screen {
            // `xyToLogicalPosition`: the first line of the fold if the top is on one
            let line = layout.y_to_logical_line(top).min(layout.line_count() - 1);
            (line, layout.line_top(line) - top)
        } else {
            (cursor_line, cursor_y - top)
        };
        Self {
            anchor,
            keeps_cursor: !off_screen,
        }
    }

    /// The scroll y after the change. `cursor_line` is after the change, moved out of closed folds.
    pub(crate) fn restore(
        &self,
        layout: &LineLayout,
        cursor_line: usize,
        viewport: Viewport,
    ) -> f32 {
        let (line, shift) = self.anchor;
        let line = if self.keeps_cursor { cursor_line } else { line };
        layout.clamp_scroll(layout.line_top(line) - shift, viewport.height)
    }
}
