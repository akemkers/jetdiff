//! The walkthrough mode in the repo view: `w` shows it in place of the side panel and the diff, and
//! back. It is kept while the same change is shown, and while it is being written.

use std::rc::Rc;

use gpui::{Context, Entity, Focusable, Window, prelude::*};
use jetdiff_files::ChangedFile;

use super::RepoView;
use crate::walkthrough_mode::{WalkthroughEvent, WalkthroughMode};

/// The walkthrough mode and the change it was opened for.
pub(super) struct Walkthrough {
    mode: Entity<WalkthroughMode>,
    what: String,
    changed_files: Rc<[ChangedFile]>,
    pub(super) shown: bool,
    _events: gpui::Subscription,
}

impl RepoView {
    pub(super) fn toggle_walkthrough(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            same || walkthrough.mode.read(cx).is_writing()
        });
        if !reuse {
            let repo = self.repo_source.repo().clone();
            let files = changed_files.to_vec();
            let (description, scale) = (what.clone(), self.scale);
            let mode = cx.new(|cx| WalkthroughMode::new(repo, files, description, scale, cx));
            let events = cx.subscribe_in(&mode, window, |this, _, event, window, cx| match event {
                WalkthroughEvent::Close => this.close_walkthrough(window, cx),
                WalkthroughEvent::Open { path, hunk } => {
                    this.close_walkthrough(window, cx);
                    this.open(path, *hunk, cx);
                }
            });
            self.walkthrough = Some(Walkthrough {
                mode,
                what,
                changed_files,
                shown: false,
                _events: events,
            });
        }
        if let Some(walkthrough) = &mut self.walkthrough {
            walkthrough.shown = true;
            window.focus(&walkthrough.mode.focus_handle(cx));
        }
        cx.notify();
    }

    pub(super) fn walkthrough_mode(&self) -> Option<Entity<WalkthroughMode>> {
        let walkthrough = self
            .walkthrough
            .as_ref()
            .filter(|walkthrough| walkthrough.shown)?;
        Some(walkthrough.mode.clone())
    }

    pub(super) fn walkthrough_zoomed(&self, cx: &mut Context<Self>) {
        if let Some(walkthrough) = &self.walkthrough {
            let scale = self.scale;
            walkthrough
                .mode
                .update(cx, |mode, cx| mode.set_scale(scale, cx));
        }
    }

    fn close_walkthrough(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(walkthrough) = &mut self.walkthrough {
            walkthrough.shown = false;
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

    /// For the model: what the change is.
    fn what(&self, cx: &gpui::App) -> String {
        match &self.commits {
            Some(commits) => commits.read(cx).describe_selected(),
            None => format!("The change in {}", self.repo_source.repo().name()),
        }
    }
}
