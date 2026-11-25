//! A File pair: the diff of the two files, read again on a source reload.

use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, Window, div, prelude::*};
use jetdiff_app::FilePair;
use jetdiff_viewmodel::FileDiff;

use crate::file_diff::FileDiffView;
use crate::key_bindings::KEY_CONTEXT;
use crate::notification_slot_view::NotificationSlotView;
use crate::repo_view::SourceReload;

pub struct FilePairView {
    file_pair: FilePair,
    diff: Entity<FileDiffView>,
    notification_slot: Entity<NotificationSlotView>,
}

impl FilePairView {
    pub fn new(file_pair: FilePair, diff: FileDiff, cx: &mut Context<Self>) -> Self {
        let notification_slot = cx.new(|_| NotificationSlotView::default());
        let paths = file_pair.paths().clone();
        let slot = notification_slot.clone();
        let diff = cx.new(|cx| FileDiffView::standalone(diff, paths, slot, cx));
        Self {
            file_pair,
            diff,
            notification_slot,
        }
    }

    /// The cursor and scroll are kept, as for the same file in a repo source.
    fn source_reload(&mut self, _: &SourceReload, _: &mut Window, cx: &mut Context<Self>) {
        let (diff, brief) = self.file_pair.source_reload();
        if let Some(diff) = diff {
            let paths = self.file_pair.paths().clone();
            self.diff
                .update(cx, |view, cx| view.update_diff(diff, paths, cx));
        }
        self.notification_slot
            .update(cx, |slot, cx| slot.show_brief(brief, cx));
    }
}

impl Focusable for FilePairView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.diff.focus_handle(cx)
    }
}

impl Render for FilePairView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .on_action(cx.listener(Self::source_reload))
            .child(self.diff.clone())
    }
}
