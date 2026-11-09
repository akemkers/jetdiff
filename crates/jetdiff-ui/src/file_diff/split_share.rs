//! The share of the width Left gets in Split, and the handle over the diff divider that drags it.

use gpui::{AnyElement, Context, Div, prelude::*};
use jetdiff_viewmodel::{DiffMode, SplitShare, diff_divider_offset};

use super::FileDiffView;
use crate::diff_divider_handle::with_diff_divider_handle;

impl FileDiffView {
    pub(crate) fn split_share(&self) -> SplitShare {
        self.split_share
    }

    /// The text width changes with the share, so horizontal scroll is kept within it.
    pub(crate) fn set_split_share(&mut self, share: SplitShare, cx: &mut Context<Self>) {
        if share == self.split_share {
            return;
        }
        self.split_share = share;
        self.scroll.stop(self.mode.scroll_mut());
        self.jump_to_clamped_scroll();
        cx.notify();
    }

    /// `code` fills the area, with the handle over the diff divider in Split.
    pub(super) fn with_split_share_handle(&self, code: AnyElement, cx: &mut Context<Self>) -> Div {
        with_diff_divider_handle(
            "diff-divider".into(),
            code,
            self.diff_divider_offset(),
            cx,
            Self::set_split_share,
        )
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
    }

    /// From the left edge of the code, with the width it was drawn with last. `None` in Unified and
    /// before it is drawn.
    fn diff_divider_offset(&self) -> Option<f32> {
        let bounds = self
            .shared
            .bounds
            .get()
            .filter(|_| matches!(self.mode, DiffMode::Split(_)))?;
        Some(diff_divider_offset(
            f32::from(bounds.size.width),
            self.split_share,
        ))
    }
}
