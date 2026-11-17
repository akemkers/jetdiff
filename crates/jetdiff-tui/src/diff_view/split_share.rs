//! The share of the width Left gets in Split, changed by dragging the diff divider with the mouse.

use std::time::Instant;

use jetdiff_viewmodel::SplitShare;

use super::DiffView;
use crate::diff_divider_grab::grab_offset;
use crate::geometry;

impl DiffView {
    /// A press on the diff divider or beside it grabs it, and a double click gives 50/50 again.
    /// `false` when the press is somewhere else.
    pub(super) fn grab_diff_divider(&mut self, x: u16, y: u16, now: Instant) -> bool {
        let grab = self
            .diff_divider()
            .and_then(|divider| grab_offset(divider, x))
            .filter(|_| self.area.contains((x, y).into()));
        let Some(offset) = grab else {
            self.diff_divider_grab.press_elsewhere();
            return false;
        };
        if self.diff_divider_grab.press(now, offset) {
            self.set_split_share(SplitShare::default());
        }
        true
    }

    pub fn is_dragging_diff_divider(&self) -> bool {
        self.diff_divider_grab.is_dragging()
    }

    /// The mouse is in column `x` while the diff divider is dragged.
    pub fn drag_diff_divider(&mut self, x: u16) {
        let column = self.diff_divider_grab.drag_to(x);
        self.set_split_share(geometry::split_share_at(self.area, column));
    }

    pub fn release_diff_divider(&mut self) {
        self.diff_divider_grab.release();
    }

    /// A press outside the diff, in the side panel.
    pub fn press_elsewhere(&mut self) {
        self.diff_divider_grab.press_elsewhere();
    }

    /// The column of the diff divider, `None` in Unified.
    fn diff_divider(&self) -> Option<u16> {
        (!self.mode.is_unified())
            .then(|| geometry::aligned(self.mode.diff(), self.area, self.split_share).divider)
    }

    /// The text width changes with the share, so horizontal scroll is kept within it.
    fn set_split_share(&mut self, share: SplitShare) {
        self.split_share = share;
        self.clamp_scroll_x();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Theme};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::*;

    #[test]
    fn the_diff_divider_is_dragged_to_both_edges() {
        let diff = FileDiff::new("a\nb\n", "a\nc\n", ComparisonPolicy::Default).unwrap();
        let mut view = DiffView::new(diff, Default::default(), false, false, false);
        let area = Rect::new(0, 0, 40, 5);
        view.layout(area);
        let now = Instant::now();
        view.click(view.diff_divider().unwrap(), 0, now);
        assert!(view.is_dragging_diff_divider());
        // The last column is the scrollbar
        for (x, divider) in [(0, 0), (10, 10), (39, 38)] {
            view.drag_diff_divider(x);
            assert_eq!(view.diff_divider(), Some(divider));
            view.render(true, &Theme::built_in(), &mut Buffer::empty(area));
        }
        view.release_diff_divider();
        assert!(!view.is_dragging_diff_divider());
        // Grabbed beside it, the divider keeps its distance to the mouse
        view.click(39, 0, now);
        view.drag_diff_divider(30);
        assert_eq!(view.diff_divider(), Some(29));
        view.release_diff_divider();
        let divider = view.diff_divider().unwrap();
        view.click(divider, 0, now);
        view.click(divider, 0, now + Duration::from_millis(100));
        assert_eq!(view.split_share, SplitShare::default());
    }
}
