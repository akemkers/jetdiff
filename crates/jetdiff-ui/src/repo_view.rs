//! The side panel and the diff next to each other. Loads the selected file, and reads the repo source
//! again on a source reload, when a row in the commit list is selected, a hunk is rolled back,
//! files change on disk or HEAD moves. `w` shows the walkthrough in place of the diff, and its
//! contents in the side panel. The keymap bar spans the window under both.

mod side_panel;
mod walkthrough;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, Render, Subscription, Task, Window,
    actions, div, prelude::*,
};
use jetdiff_app::help::{DIFF_KEYMAP_BAR, WALKTHROUGH_KEYMAP_BAR};
use jetdiff_app::{
    Applied, Commits, DiskChange, Notification, ReadJob, RepoSource, Show, Shown, empty_diff,
};
use jetdiff_viewmodel::metrics::FONT_SIZE;

use crate::commit_list::{CommitList, SelectRow};
use crate::confirm_dialog::confirm_dialog;
use crate::file_diff::{FileDiffView, ResetZoom, Rollback, ZoomIn, ZoomOut, Zoomed};
use crate::file_tree::{FileTree, Selection};
use crate::help::{HideHelp, ToggleHelp, keymap_bar};
use crate::notification_slot_view::NotificationSlotView;
use crate::settings::Settings;
use crate::side_panel_size::{SectionHeight, SidePanelWidth};
use crate::watcher::Watcher;

// Vim's window keys, and file and commit switching. The tree and the commit list are on the left, the diff on the right.
actions!(
    jetdiff,
    [
        FocusLeft,
        FocusDown,
        FocusUp,
        FocusRight,
        ToggleSidePanel,
        NextFile,
        PrevFile,
        NextCommit,
        PrevCommit,
        ToggleWalkthrough,
        SourceReload
    ]
);

pub struct RepoView {
    repo_source: RepoSource,
    selection: Entity<Selection>,
    files: Entity<FileTree>,
    diff: Entity<FileDiffView>,
    notification_slot: Entity<NotificationSlotView>,
    /// Only for a folder, not for a commit or a PR.
    commits: Option<Entity<CommitList>>,
    walkthrough: Option<walkthrough::OpenWalkthrough>,
    /// The hunk the diff lands on once the file opened from the walkthrough is loaded.
    landing: Option<usize>,
    /// The popup over the diff, about the walkthrough.
    question: Option<walkthrough::Question>,
    scale: f32,
    side_panel_open: bool,
    /// The sections in the side panel can be closed to just the heading.
    files_open: bool,
    commits_open: bool,
    width: SidePanelWidth,
    commits_height: SectionHeight,
    _watcher: Option<Watcher>,
    _read: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl RepoView {
    pub fn new(repo_source: RepoSource, commits: Option<Commits>, cx: &mut Context<Self>) -> Self {
        let first = repo_source.first();
        let selection = cx.new(|_| Selection(first));
        let files =
            cx.new(|cx| FileTree::new(repo_source.changed_files().clone(), selection.clone(), cx));
        let notification_slot = cx.new(|_| NotificationSlotView::default());
        let diff = cx.new(|cx| {
            FileDiffView::new(
                empty_diff(),
                Default::default(),
                notification_slot.clone(),
                cx,
            )
        });
        let mut subscriptions = vec![
            cx.observe(&selection, |this, _, cx| this.load(cx)),
            cx.subscribe(&diff, |this, _, e: &Zoomed, cx| {
                this.zoomed(e.font_size, cx)
            }),
            cx.subscribe(&diff, |this, _, e: &Rollback, cx| this.rollback(e, cx)),
        ];

        let (mut commit_list, mut watcher) = (None, None);
        if let Some(commits) = commits {
            let list = cx.new(|cx| CommitList::new(commits, cx));
            subscriptions.push(cx.subscribe(&list, |this, _, e: &SelectRow, cx| {
                let job = this.repo_source.select(e.0.clone());
                this.read(job, cx);
            }));
            commit_list = Some(list);
            match Watcher::new(repo_source.repo(), cx, Self::changed_on_disk) {
                Ok(w) => watcher = Some(w),
                Err(e) => {
                    let brief = Notification::warning(format!("Can't watch for file changes: {e}"));
                    notification_slot.update(cx, |slot, cx| slot.show_brief(brief, cx));
                }
            }
        }

        let mut view = Self {
            repo_source,
            selection,
            files,
            diff,
            notification_slot,
            commits: commit_list,
            walkthrough: None,
            landing: None,
            question: None,
            scale: 1.,
            side_panel_open: cx.global::<Settings>().side_panel_open,
            files_open: true,
            commits_open: true,
            width: SidePanelWidth::default(),
            commits_height: SectionHeight::default(),
            _watcher: watcher,
            _read: None,
            _subscriptions: subscriptions,
        };
        view.zoomed(cx.global::<Settings>().font_size, cx);
        view.load(cx);
        view
    }

    fn zoomed(&mut self, font_size: f32, cx: &mut Context<Self>) {
        self.scale = font_size / FONT_SIZE;
        let scale = self.scale;
        self.files.update(cx, |t, cx| t.set_scale(scale, cx));
        if let Some(commits) = &self.commits {
            commits.update(cx, |c, cx| c.set_scale(scale, cx));
        }
        self.walkthrough_zoomed(cx);
    }

    /// Shows the file after the rollback and reads the repo source again, without waiting for the watcher.
    fn rollback(&mut self, e: &Rollback, cx: &mut Context<Self>) {
        match self.repo_source.rollback(&e.0) {
            None => {}
            Some(Ok(show)) => {
                self.show(show, cx);
                let job = self.repo_source.rolled_back();
                self.read(job, cx);
            }
            Some(Err(text)) => self.notification_slot.update(cx, |slot, cx| {
                slot.show_brief(Notification::warning(text), cx)
            }),
        }
    }

    /// Also without a watcher, and also in the walkthrough, where only the diff behind it changes.
    fn source_reload(&mut self, _: &SourceReload, _: &mut Window, cx: &mut Context<Self>) {
        let job = self.repo_source.source_reload();
        self.read(job, cx);
    }

    fn changed_on_disk(&mut self, change: DiskChange, cx: &mut Context<Self>) {
        let job = self.repo_source.changed_on_disk(change);
        self.read(job, cx);
    }

    /// Git is read in the background. A new read cancels the one on the way.
    fn read(&mut self, job: ReadJob, cx: &mut Context<Self>) {
        self._read = Some(cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move { job.run() })
                .await;
            this.update(cx, |view, cx| {
                let applied = view.repo_source.apply(read);
                view.show_applied(applied, cx)
            })
            .ok();
        }));
    }

    fn show_applied(&mut self, applied: Applied, cx: &mut Context<Self>) {
        match applied {
            Applied::Failed { selected, brief } => {
                // The list goes back to what is still shown
                if let Some(list) = &self.commits {
                    list.update(cx, |commit_list, cx| commit_list.set_selected(selected, cx));
                }
                self.notification_slot
                    .update(cx, |slot, cx| slot.show_brief(brief, cx));
            }
            Applied::Done {
                branch_name,
                commits,
                selected,
                changed_files,
                file,
                brief,
            } => {
                if let Some(list) = &self.commits {
                    list.update(cx, |commit_list, cx| {
                        commit_list.set_commits(branch_name, commits, selected, cx)
                    });
                }
                if let Some(changed_files) = changed_files {
                    self.files.update(cx, |file_tree, cx| {
                        file_tree.set_changed_files(changed_files, cx)
                    });
                }
                self.selection
                    .update(cx, |selection, cx| selection.set(file, cx));
                if let Some(brief) = brief {
                    self.notification_slot
                        .update(cx, |slot, cx| slot.show_brief(brief, cx));
                }
            }
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let show = self.repo_source.load(self.selection.read(cx).0);
        self.show(show, cx);
        if let Some(hunk) = self.landing.take() {
            self.diff.update(cx, |diff, cx| diff.land_on_hunk(hunk, cx));
        }
    }

    /// If it is the same file as before, with new content, the cursor and scroll are kept.
    fn show(&mut self, show: Show, cx: &mut Context<Self>) {
        match show {
            Show::Unchanged => return,
            Show::Empty { pinned } => {
                self.notification_slot
                    .update(cx, |slot, cx| slot.pin(Some(pinned), cx));
                self.diff
                    .update(cx, |v, cx| v.set_diff(empty_diff(), Default::default(), cx));
            }
            Show::Diff(shown) => {
                let Shown {
                    diff,
                    pinned,
                    paths,
                    can_rollback,
                    same_file,
                } = *shown;
                self.notification_slot
                    .update(cx, |slot, cx| slot.pin(pinned, cx));
                self.diff.update(cx, |v, cx| {
                    v.set_can_rollback(can_rollback);
                    if same_file {
                        v.update_diff(diff, paths, cx)
                    } else {
                        v.set_diff(diff, paths, cx)
                    }
                });
            }
        }
        cx.notify();
    }

    /// The tree and the commit list, only the ones that are open.
    fn open_panes(&self, cx: &App) -> (Option<FocusHandle>, Option<FocusHandle>) {
        if !self.side_panel_open {
            return (None, None);
        }
        let tree = self.files_open.then(|| self.files.focus_handle(cx));
        let commits = self.commits.as_ref().filter(|_| self.commits_open);
        (tree, commits.map(|c| c.focus_handle(cx)))
    }

    fn toggle_side_panel(
        &mut self,
        _: &ToggleSidePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.side_panel_open = !self.side_panel_open;
        if !self.side_panel_open && self.walkthrough_view().is_none() {
            window.focus(&self.diff.focus_handle(cx));
        }
        cx.notify();
    }

    fn focus_left(&mut self, _: &FocusLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.diff.focus_handle(cx).is_focused(window) {
            let (tree, commits) = self.open_panes(cx);
            if let Some(pane) = tree.or(commits) {
                window.focus(&pane);
            }
        }
    }

    fn focus_right(&mut self, _: &FocusRight, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.diff.focus_handle(cx));
    }

    fn focus_down(&mut self, _: &FocusDown, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(tree), Some(commits)) = self.open_panes(cx)
            && tree.is_focused(window)
        {
            window.focus(&commits);
        }
    }

    fn focus_up(&mut self, _: &FocusUp, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(tree), Some(commits)) = self.open_panes(cx)
            && commits.is_focused(window)
        {
            window.focus(&tree);
        }
    }
}

impl Focusable for RepoView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.diff.focus_handle(cx)
    }
}

impl Render for RepoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let walkthrough = self.walkthrough_view();
        let theme = cx.global::<Settings>().theme;
        let question = self
            .question
            .map(|question| confirm_dialog(self.question_text(question, cx), &theme.palette));
        let keys = match walkthrough {
            Some(_) => WALKTHROUGH_KEYMAP_BAR.as_slice(),
            None => DIFF_KEYMAP_BAR.as_slice(),
        };
        let keymap_bar = keymap_bar(
            keys,
            self.notification_slot.clone(),
            &theme.palette,
            self.scale,
        );
        let main: AnyElement = match walkthrough {
            Some(walkthrough) => walkthrough.into_any_element(),
            None => self.diff.clone().into_any_element(),
        };
        let content = div()
            .size_full()
            .flex()
            .when(self.side_panel_open, |d| {
                d.child(self.side_panel(window, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().min_h_0().child(main)),
            );
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(Self::answer))
            .on_action(cx.listener(|this, _: &ToggleWalkthrough, window, cx| {
                this.toggle_walkthrough(window, cx)
            }))
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::toggle_side_panel))
            .on_action(cx.listener(Self::source_reload))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| {
                this.files.update(cx, |t, cx| t.step(true, cx))
            }))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| {
                this.files.update(cx, |t, cx| t.step(false, cx))
            }))
            .on_action(cx.listener(|this, _: &NextCommit, _, cx| {
                if let Some(list) = &this.commits {
                    list.update(cx, |c, cx| c.step(true, cx))
                }
            }))
            .on_action(cx.listener(|this, _: &PrevCommit, _, cx| {
                if let Some(list) = &this.commits {
                    list.update(cx, |c, cx| c.step(false, cx))
                }
            }))
            // Zoom, help and diff mode also work when the tree or the commit list has focus
            .on_action(cx.listener(|this, a: &ZoomIn, w, cx| {
                this.diff.update(cx, |d, cx| d.zoom_in(a, w, cx))
            }))
            .on_action(cx.listener(|this, a: &ZoomOut, w, cx| {
                this.diff.update(cx, |d, cx| d.zoom_out(a, w, cx))
            }))
            .on_action(cx.listener(|this, a: &ResetZoom, w, cx| {
                this.diff.update(cx, |d, cx| d.reset_zoom(a, w, cx))
            }))
            .on_action(cx.listener(|this, a: &ToggleHelp, w, cx| {
                this.diff.update(cx, |d, cx| d.toggle_help(a, w, cx))
            }))
            .on_action(cx.listener(|this, a: &HideHelp, w, cx| {
                if this.question.take().is_some() {
                    return cx.notify();
                }
                this.diff.update(cx, |d, cx| d.hide_help(a, w, cx))
            }))
            .on_action(cx.listener(Self::toggle_diff_mode))
            .child(div().flex_1().min_h_0().child(content))
            .child(keymap_bar)
            .children(question)
    }
}
