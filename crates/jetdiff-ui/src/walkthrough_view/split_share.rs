//! The share of the width Left gets in the excerpts, the same as in the diff. Dragging the diff
//! divider in an excerpt changes it for all of them.

use gpui::{AnyElement, Context, Div, prelude::*};
use jetdiff_viewmodel::SplitShare;

use super::WalkthroughView;
use crate::code_viewer::Excerpt;
use crate::diff_divider_handle::with_diff_divider_handle;

impl WalkthroughView {
    pub fn split_share(&self) -> SplitShare {
        self.split_share
    }

    pub fn set_split_share(&mut self, share: SplitShare, cx: &mut Context<Self>) {
        if share != self.split_share {
            self.split_share = share;
            cx.notify();
        }
    }

    /// `code` draws `excerpt` from the page's item `item`, with the handle over the diff divider in
    /// Split.
    pub(super) fn with_split_share_handle(
        &self,
        item: usize,
        excerpt: &Excerpt,
        code: AnyElement,
        cx: &mut Context<Self>,
    ) -> Div {
        with_diff_divider_handle(
            ("diff-divider", item).into(),
            code,
            excerpt.diff_divider_offset(self.split_share),
            cx,
            Self::set_split_share,
        )
        .w_full()
    }
}
