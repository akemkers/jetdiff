//! The walkthrough in the repo view: `w` shows it in place of the side panel and the diff, and
//! back. Without one, `w` asks whether Claude should write it, and the notification slot shows the
//! writing while the diff is read. It is kept while the same diff is shown, and while it is written.

use std::rc::Rc;
use std::time::Instant;

use gpui::{Context, Entity, Focusable, KeyDownEvent, SharedString, Window, prelude::*};
use jetdiff_app::{Notification, Progress};
use jetdiff_files::ChangedFile;

use super::RepoView;
use crate::file_diff::ToggleDiffMode;
use crate::settings::Settings;
use crate::walkthrough_view::{Status, WalkthroughEvent, WalkthroughView};

/// The walkthrough view and the diff it was opened for.
pub(super) struct OpenWalkthrough {
    view: Entity<WalkthroughView>,
    what: String,
    changed_files: Rc<[ChangedFile]>,
    pub(super) shown: bool,
    /// `w` was pressed while the diff was read, so it is shown or asked about when it is.
    opening: bool,
    _events: gpui::Subscription,
    /// Its contents are drawn in the side panel, so the repo view draws again with it.
    _redraw: gpui::Subscription,
}

/// What the popup over the diff asks. Enter answers yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Question {
    Write,
    Stop,
}

impl RepoView {
    pub(super) fn toggle_walkthrough(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.question.take().is_some() {
            return cx.notify();
        }
        if self
            .walkthrough
            .as_ref()
            .is_some_and(|walkthrough| walkthrough.shown)
        {
            return self.close_walkthrough(window, cx);
        }
        let what = self.what(cx);
        let changed_files = self.repo_source.changed_files().clone();
        let reuse = self.walkthrough.as_ref().is_some_and(|walkthrough| {
            let same =
                walkthrough.what == what && Rc::ptr_eq(&walkthrough.changed_files, &changed_files);
            same || walkthrough.view.read(cx).is_writing()
        });
        if !reuse {
            let repo = self.repo_source.repo().clone();
            let files = changed_files.to_vec();
            let (description, scale) = (what.clone(), self.scale);
            let view = cx.new(|cx| WalkthroughView::new(repo, files, description, scale, cx));
            let events = cx.subscribe_in(&view, window, Self::walkthrough_event);
            let redraw = cx.observe(&view, |_, _, cx| cx.notify());
            self.walkthrough = Some(OpenWalkthrough {
                view,
                what,
                changed_files,
                shown: false,
                opening: false,
                _events: events,
                _redraw: redraw,
            });
        }
        self.open_walkthrough(window, cx);
    }

    pub(super) fn walkthrough_view(&self) -> Option<Entity<WalkthroughView>> {
        let walkthrough = self
            .walkthrough
            .as_ref()
            .filter(|walkthrough| walkthrough.shown)?;
        Some(walkthrough.view.clone())
    }

    pub(super) fn walkthrough_zoomed(&self, cx: &mut Context<Self>) {
        if let Some(walkthrough) = &self.walkthrough {
            let scale = self.scale;
            walkthrough
                .view
                .update(cx, |view, cx| view.set_scale(scale, cx));
        }
    }

    /// `v` in the walkthrough switches it and the diff, also when the diff shows a file that is
    /// always Unified. A hidden walkthrough follows the diff when it is shown again.
    pub(super) fn toggle_diff_mode(
        &mut self,
        action: &ToggleDiffMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.walkthrough_view() {
            Some(view) => {
                let unified = !self.diff.read(cx).is_unified();
                self.diff
                    .update(cx, |diff, cx| diff.set_unified(unified, cx));
                view.update(cx, |view, cx| view.set_unified(unified, cx));
            }
            None => self
                .diff
                .update(cx, |diff, cx| diff.toggle_diff_mode(action, window, cx)),
        }
    }

    /// The popup's question, with the model that would write it.
    pub(super) fn question_text(&self, question: Question, cx: &gpui::App) -> SharedString {
        let model = &cx.global::<Settings>().walkthrough.model;
        match question {
            Question::Write => format!(
                "Write a step-by-step walkthrough of this diff with Claude {model}? It can take a few minutes."
            )
            .into(),
            Question::Stop => "Stop writing the walkthrough?".into(),
        }
    }

    /// While the popup is open, Enter answers yes and any other key no. Esc and `w` come as
    /// actions, which close it too.
    pub(super) fn answer(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(question) = self.question.take() else {
            return;
        };
        cx.stop_propagation();
        let keystroke = &ev.keystroke;
        if keystroke.key == "enter" && !keystroke.modifiers.modified() {
            match question {
                Question::Write => self.write_walkthrough(cx),
                Question::Stop => self.stop_walkthrough(cx),
            }
        }
        cx.notify();
    }

    /// Shown when it is written, asked about when it can be, and told why when it can't.
    fn open_walkthrough(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(walkthrough) = &mut self.walkthrough else {
            return;
        };
        walkthrough.opening = false;
        match walkthrough.view.read(cx).status() {
            Status::Loading => walkthrough.opening = true,
            Status::Ready => {
                let diff = self.diff.read(cx);
                let (unified, split_share) = (diff.is_unified(), diff.split_share());
                walkthrough.view.update(cx, |view, cx| {
                    view.set_unified(unified, cx);
                    view.set_split_share(split_share, cx);
                });
                walkthrough.shown = true;
                window.focus(&walkthrough.view.focus_handle(cx));
            }
            Status::CanWrite => self.question = Some(Question::Write),
            Status::Writing => self.question = Some(Question::Stop),
            Status::CantWrite(reason) => self.show_brief(Notification::info(reason), cx),
        }
        cx.notify();
    }

    fn walkthrough_event(
        &mut self,
        _: &Entity<WalkthroughView>,
        event: &WalkthroughEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WalkthroughEvent::Loaded => {
                if self
                    .walkthrough
                    .as_ref()
                    .is_some_and(|walkthrough| walkthrough.opening)
                {
                    self.open_walkthrough(window, cx);
                }
            }
            // It was asked for, so it opens when it is ready
            WalkthroughEvent::Written => {
                self.show_progress(None, cx);
                self.question = None;
                self.open_walkthrough(window, cx);
            }
            WalkthroughEvent::Failed(error) => {
                self.show_progress(None, cx);
                let warning = format!("The walkthrough couldn't be written: {error}");
                self.show_brief(Notification::warning(warning), cx);
            }
            WalkthroughEvent::Close => self.close_walkthrough(window, cx),
            WalkthroughEvent::Open { path, hunk } => {
                self.close_walkthrough(window, cx);
                self.open(path, *hunk, cx);
            }
        }
    }

    fn write_walkthrough(&mut self, cx: &mut Context<Self>) {
        let Some(walkthrough) = &self.walkthrough else {
            return;
        };
        walkthrough.view.update(cx, |view, cx| view.write(cx));
        let model = &cx.global::<Settings>().walkthrough.model;
        let progress = Progress::new(
            format!("Writing a walkthrough with Claude {model}…"),
            Instant::now(),
        );
        self.show_progress(Some(progress), cx);
    }

    fn stop_walkthrough(&mut self, cx: &mut Context<Self>) {
        if let Some(walkthrough) = &self.walkthrough {
            walkthrough.view.update(cx, |view, cx| view.cancel(cx));
        }
        self.show_progress(None, cx);
        self.show_brief(Notification::info("Stopped writing the walkthrough"), cx);
    }

    fn show_progress(&self, progress: Option<Progress>, cx: &mut Context<Self>) {
        self.notification_slot
            .update(cx, |slot, cx| slot.show_progress(progress, cx));
    }

    fn show_brief(&self, brief: Notification, cx: &mut Context<Self>) {
        self.notification_slot
            .update(cx, |slot, cx| slot.show_brief(brief, cx));
    }

    /// The diff gets the share the diff divider was dragged to in the walkthrough.
    fn close_walkthrough(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A walkthrough that wasn't shown never got the share from the diff
        if let Some(walkthrough) = self
            .walkthrough
            .as_mut()
            .filter(|walkthrough| walkthrough.shown)
        {
            walkthrough.shown = false;
            let split_share = walkthrough.view.read(cx).split_share();
            self.diff
                .update(cx, |diff, cx| diff.set_split_share(split_share, cx));
        }
        window.focus(&self.diff.focus_handle(cx));
        cx.notify();
    }

    /// The file is loaded when the selection is observed, and the diff lands after that.
    fn open(&mut self, path: &str, hunk: Option<usize>, cx: &mut Context<Self>) {
        let file = self
            .repo_source
            .changed_files()
            .iter()
            .position(|file| file.path == path);
        if let Some(file) = file {
            self.landing = hunk;
            self.selection
                .update(cx, |selection, cx| selection.set(Some(file), cx));
        }
    }

    /// For the model: what the diff is.
    fn what(&self, cx: &gpui::App) -> String {
        match &self.commits {
            Some(commits) => commits.read(cx).describe_selected(),
            None => format!("The change in {}", self.repo_source.repo().name()),
        }
    }
}
