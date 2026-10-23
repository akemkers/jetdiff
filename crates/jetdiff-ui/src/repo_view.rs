//! File tree and diff next to each other, with the commit list under the tree. Loads the selected file, and reads
//! the repo source again when a commit is selected, a hunk or a file is rolled back or files change on disk.

use gpui::{
    App, Context, Div, Entity, FocusHandle, Focusable, Render, Stateful, Subscription, Task,
    Window, actions, div, prelude::*, px,
};
use jetdiff_app::{
    Applied, Notification, ReadJob, RepoSource, Show, Shown, empty_diff, file_rollback_request,
};
use jetdiff_files::ChangedFile;
use jetdiff_git::Repo;
use jetdiff_viewmodel::metrics::FONT_SIZE;

use crate::commit_list::{CommitList, SelectCommit};
use crate::file_diff::{
    FileDiffView, ResetZoom, Rollback, ToggleDiffMode, ZoomIn, ZoomOut, Zoomed,
};
use crate::file_header::file_header;
use crate::file_tree::{AskFileRollback, FileRollback, FileTree, Selection};
use crate::help::{HideHelp, ToggleHelp};
use crate::list::{GAP, PAD_LEFT, ROW_HEIGHT, TEXT_SIZE};
use crate::notification_slot_view::NotificationSlotView;
use crate::rollback_dialog::rollback_dialog;
use crate::settings::Settings;
use crate::side_panel_size::{SectionHeight, SidePanelWidth};
use crate::text::FAMILY;
use crate::watcher::Watcher;

// Vim's window keys and file switching. The tree and the commit list are on the left, the diff on the right.
actions!(
    jetdiff,
    [
        FocusLeft,
        FocusDown,
        FocusUp,
        FocusRight,
        ToggleSidePanel,
        NextFile,
        PrevFile
    ]
);

pub struct RepoView {
    repo_source: RepoSource,
    selection: Entity<Selection>,
    files: Entity<FileTree>,
    diff: Entity<FileDiffView>,
    notification_slot: Entity<NotificationSlotView>,
    /// Only for Local changes.
    commits: Option<Entity<CommitList>>,
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
    pub fn new(repo: Repo, changed_files: Vec<ChangedFile>, cx: &mut Context<Self>) -> Self {
        let repo_source = RepoSource::new(repo, changed_files);
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
            cx.subscribe(&files, |this, _, _: &AskFileRollback, cx| {
                this.ask_file_rollback(cx)
            }),
            cx.subscribe(&files, |this, _, event: &FileRollback, cx| {
                this.rollback_file(event, cx)
            }),
        ];

        let (mut commits, mut watcher) = (None, None);
        let repo = repo_source.repo();
        if repo.is_local_changes() {
            let list = repo.commits().unwrap_or_else(|e| {
                notification_slot
                    .update(cx, |slot, cx| slot.show_brief(Notification::warning(e), cx));
                Vec::new()
            });
            let list = cx.new(|cx| CommitList::new(list, cx));
            subscriptions.push(cx.subscribe(&list, |this, _, e: &SelectCommit, cx| {
                let job = this.repo_source.select_commit(e.0.clone());
                this.read(job, cx);
            }));
            commits = Some(list);
            match Watcher::new(repo.root(), cx, Self::files_changed) {
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
            commits,
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

    /// Opens the dialog in the file tree, or says why the file can't be rolled back.
    fn ask_file_rollback(&mut self, cx: &mut Context<Self>) {
        let can_rollback = self.repo_source.can_rollback();
        match file_rollback_request(self.files.read(cx).files(), can_rollback) {
            Ok(request) => self.files.update(cx, |file_tree, cx| {
                file_tree.confirm_file_rollback(request, cx)
            }),
            Err(text) => self.notification_slot.update(cx, |slot, cx| {
                slot.show_brief(Notification::warning(text), cx)
            }),
        }
    }

    /// Reads the repo source again, without waiting for the watcher.
    fn rollback_file(&mut self, event: &FileRollback, cx: &mut Context<Self>) {
        match self.repo_source.rollback_file(event.0.file()) {
            Ok(()) => {
                let job = self.repo_source.rolled_back();
                self.read(job, cx);
            }
            Err(text) => self.notification_slot.update(cx, |slot, cx| {
                slot.show_brief(Notification::warning(text), cx)
            }),
        }
    }

    fn files_changed(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let job = self.repo_source.files_changed(paths);
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
            Applied::Failed {
                selected_commit,
                brief,
            } => {
                // The list goes back to the commit that is still selected
                if let Some(list) = &self.commits {
                    list.update(cx, |commit_list, cx| {
                        commit_list.set_selected(selected_commit, cx)
                    });
                }
                self.notification_slot
                    .update(cx, |slot, cx| slot.show_brief(brief, cx));
            }
            Applied::Done {
                commits,
                changed_files,
                file,
                brief,
            } => {
                if let Some(list) = &self.commits {
                    list.update(cx, |commit_list, cx| commit_list.set_commits(commits, cx));
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
        if !self.side_panel_open {
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

    /// Heading with + or −. A click opens or closes the section.
    fn heading(
        &self,
        title: &'static str,
        open: bool,
        cx: &mut Context<Self>,
        toggle: fn(&mut Self) -> &mut bool,
    ) -> Stateful<Div> {
        let s = |v: f32| px(v * self.scale);
        div()
            .id(title)
            .flex_none()
            .h(s(ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(s(GAP))
            .pl(s(PAD_LEFT))
            .text_color(cx.global::<Settings>().theme.line_number)
            .cursor_pointer()
            .child(title)
            .child(if open { "−" } else { "+" })
            .on_click(cx.listener(move |this, _, window, cx| {
                let open = toggle(this);
                *open = !*open;
                // Focus in a closed section goes to the diff
                let tree = this.files.focus_handle(cx);
                let commits = this.commits.as_ref().map(|c| c.focus_handle(cx));
                let hidden = (!this.files_open && tree.is_focused(window))
                    || (!this.commits_open && commits.is_some_and(|c| c.is_focused(window)));
                if hidden {
                    window.focus(&this.diff.focus_handle(cx));
                }
                cx.notify();
            }))
    }

    /// The tree with the commit list under it. The width follows the tree.
    fn side_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let viewport = window.viewport_size();
        let unscaled = |v: gpui::Pixels| f32::from(v) / self.scale;
        let content = self.files.update(cx, |t, _| t.content_width(window));
        let width = self.width.resolve(content, unscaled(viewport.width));
        let theme = cx.global::<Settings>().theme;
        let mut side_panel = div()
            .flex_none()
            .w(px(width * self.scale))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.editor_bg)
            .border_r_1()
            .border_color(theme.indent_guide)
            .font_family(FAMILY)
            .text_size(px(TEXT_SIZE * self.scale))
            .child(self.heading("Files", self.files_open, cx, |this| &mut this.files_open))
            .when(self.files_open, |d| {
                d.child(div().flex_1().min_h_0().child(self.files.clone()))
            });
        if let Some(commits) = self.commits.clone() {
            let section = div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(theme.indent_guide)
                .child(self.heading("Commits", self.commits_open, cx, |this| {
                    &mut this.commits_open
                }))
                .when(self.commits_open, |d| {
                    d.child(div().flex_1().min_h_0().child(commits))
                });
            // When closed, the section moves to the bottom. Without the tree, the commit list takes the rest.
            side_panel = match (self.files_open, self.commits_open) {
                (true, true) => {
                    let height = self.commits_height.resolve(unscaled(viewport.height));
                    SectionHeight::attach(
                        side_panel,
                        section.h(px(height * self.scale)),
                        cx,
                        |this| (&mut this.commits_height, this.scale),
                    )
                }
                (false, true) => side_panel.child(section.flex_1().min_h_0()),
                (true, false) => side_panel.child(section),
                (false, false) => side_panel.child(div().flex_1()).child(section),
            };
        }
        SidePanelWidth::attach(side_panel, cx, |this| (&mut this.width, this.scale))
    }
}

impl Focusable for RepoView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.diff.focus_handle(cx)
    }
}

impl Render for RepoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let file_rollback_dialog = self
            .files
            .read(cx)
            .file_rollback_question()
            .map(|question| rollback_dialog(question, &cx.global::<Settings>().theme));
        div()
            .relative()
            .size_full()
            .flex()
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::toggle_side_panel))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| {
                this.files.update(cx, |t, cx| t.step(true, cx))
            }))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| {
                this.files.update(cx, |t, cx| t.step(false, cx))
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
                this.diff.update(cx, |d, cx| d.hide_help(a, w, cx))
            }))
            .on_action(cx.listener(|this, action: &ToggleDiffMode, window, cx| {
                this.diff
                    .update(cx, |diff, cx| diff.toggle_diff_mode(action, window, cx))
            }))
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
                    .children(self.repo_source.shown().map(|file| {
                        file_header(&file.path, self.scale, &cx.global::<Settings>().theme)
                    }))
                    .child(div().flex_1().min_h_0().child(self.diff.clone())),
            )
            .children(file_rollback_dialog)
    }
}
