//! Filtre og diff side om side, med commitene under treet. Henter innholdet fra git når en
//! fil eller commit velges, og på nytt når working tree endrer seg.

use std::rc::Rc;

use gpui::{
    App, Context, Div, Entity, FocusHandle, Focusable, Render, Stateful, Subscription, Task,
    Window, actions, div, prelude::*, px,
};
use jetdiff_changes::FileChange;
use jetdiff_git::{Commit, Repo};
use jetdiff_view::metrics::FONT_SIZE;
use jetdiff_view::{ComparisonPolicy, FileDiff, Language};

use crate::changes_tree::{ChangesTree, Selection};
use crate::commit_list::{CommitList, SelectCommit};
use crate::file_diff::{FileDiffView, ResetZoom, Rollback, ZoomIn, ZoomOut, Zoomed};
use crate::help::{HideHelp, ToggleHelp};
use crate::list::{GAP, PAD_LEFT, ROW_HEIGHT, TEXT_SIZE};
use crate::message::{Level, Message};
use crate::panel_size::{PanelHeight, PanelWidth};
use crate::theme::UiTheme;
use crate::watcher::Watcher;

// Vim sine vindustaster og filbytte. Treet og commitene er til venstre, diffen til høyre.
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

/// Det som hentes fra git i bakgrunnen.
struct Loaded {
    commit: Option<String>,
    repo: Repo,
    changes: Vec<FileChange>,
    label: String,
    commits: Vec<Commit>,
}

pub struct ChangesView {
    repo: Repo,
    /// Commiten som skal vises. `None` er working tree.
    commit: Option<String>,
    /// Commiten `changes` er fra. Blir lik `commit` når lastingen er ferdig.
    loaded_commit: Option<String>,
    changes: Rc<[FileChange]>,
    selection: Entity<Selection>,
    tree: Entity<ChangesTree>,
    diff: Entity<FileDiffView>,
    message: Entity<Message>,
    /// Bare for working tree.
    commits: Option<Entity<CommitList>>,
    theme: UiTheme,
    font_family: &'static str,
    scale: f32,
    panel_open: bool,
    /// Seksjonene i sidepanelet kan klappes sammen til bare overskriften.
    files_open: bool,
    commits_open: bool,
    width: PanelWidth,
    commits_height: PanelHeight,
    /// Det diffen viser nå. Samme innhold etter en reload lar posisjonen stå.
    shown: Option<(FileChange, Result<[String; 2], String>)>,
    _watcher: Option<Watcher>,
    _reload: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ChangesView {
    pub fn new(
        repo: Repo,
        changes: Vec<FileChange>,
        theme: UiTheme,
        font_family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let changes: Rc<[FileChange]> = changes.into();
        let first = jetdiff_changes::ChangeTree::new(&changes).first();
        let selection = cx.new(|_| Selection(first));
        let tree = cx
            .new(|cx| ChangesTree::new(changes.clone(), selection.clone(), theme, font_family, cx));
        let message = cx.new(|_| Message::new(theme, font_family));
        let diff = cx.new(|cx| {
            FileDiffView::new(
                empty_diff(),
                Default::default(),
                message.clone(),
                theme,
                font_family,
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

        let (mut commits, mut watcher) = (None, None);
        if repo.is_work_tree() {
            let list = repo.commits().unwrap_or_else(|e| {
                message.update(cx, |m, cx| m.show(e, Level::Warning, cx));
                Vec::new()
            });
            let list = cx.new(|cx| CommitList::new(repo.label(), list, theme, font_family, cx));
            subscriptions.push(cx.subscribe(&list, |this, _, e: &SelectCommit, cx| {
                this.commit = e.0.clone();
                this.changed(Vec::new(), cx);
            }));
            commits = Some(list);
            match Watcher::new(repo.root(), cx, Self::changed) {
                Ok(w) => watcher = Some(w),
                Err(e) => {
                    let text = format!("Følger ikke endringer: {e}");
                    message.update(cx, |m, cx| m.show(text, Level::Warning, cx));
                }
            }
        }

        let mut view = Self {
            repo,
            commit: None,
            loaded_commit: None,
            changes,
            selection,
            tree,
            diff,
            message,
            commits,
            theme,
            font_family,
            scale: 1.,
            panel_open: true,
            files_open: true,
            commits_open: true,
            width: PanelWidth::default(),
            commits_height: PanelHeight::default(),
            shown: None,
            _watcher: watcher,
            _reload: None,
            _subscriptions: subscriptions,
        };
        view.load(cx);
        view
    }

    fn zoomed(&mut self, font_size: f32, cx: &mut Context<Self>) {
        self.scale = font_size / FONT_SIZE;
        let scale = self.scale;
        self.tree.update(cx, |t, cx| t.set_scale(scale, cx));
        if let Some(commits) = &self.commits {
            commits.update(cx, |c, cx| c.set_scale(scale, cx));
        }
    }

    /// Oppdaterer diffen og lista med en gang, uten å vente på watcheren.
    fn rollback(&mut self, e: &Rollback, cx: &mut Context<Self>) {
        let Some((change, Ok([left, _]))) = self.shown.clone() else {
            return;
        };
        match self.repo.rollback(&change, e.lines.clone(), &e.before) {
            Ok(after) => {
                self.show(change, Ok([left, after]), cx);
                self.changed(Vec::new(), cx);
            }
            Err(text) => self
                .message
                .update(cx, |m, cx| m.show(text, Level::Warning, cx)),
        }
    }

    /// Git leses i bakgrunnen. En ny endring underveis avbryter den forrige. Commit-lista
    /// hentes på nytt hver gang, siden working tree bestemmer hvilke commits den har.
    fn changed(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let root = self.repo.root().to_owned();
        let commit = self.commit.clone();
        self._reload = Some(cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let home = Repo::work_tree(&root)?;
                    let label = home.label();
                    let commits = home.commits()?;
                    let repo = match &commit {
                        Some(hash) => Repo::commit(&root, hash)?,
                        None => home,
                    };
                    let changes = repo.changes()?;
                    Ok(Loaded {
                        commit,
                        repo,
                        changes,
                        label,
                        commits,
                    })
                })
                .await;
            this.update(cx, |v, cx| v.reloaded(loaded, &paths, cx)).ok();
        }));
    }

    fn reloaded(
        &mut self,
        loaded: Result<Loaded, String>,
        paths: &[String],
        cx: &mut Context<Self>,
    ) {
        let Loaded {
            commit,
            repo,
            changes,
            label,
            commits,
        } = match loaded {
            Ok(l) => l,
            Err(e) => {
                // Lista skal vise det som faktisk vises
                self.commit = self.loaded_commit.clone();
                if let Some(list) = &self.commits {
                    let shown = self.commit.clone();
                    list.update(cx, |l, cx| l.set_selected(shown, cx));
                }
                self.message
                    .update(cx, |m, cx| m.show(e, Level::Warning, cx));
                return;
            }
        };
        if let Some(list) = &self.commits {
            list.update(cx, |l, cx| l.set_commits(label, commits, cx));
        }
        // Bare working tree mot working tree er en oppdatering. En commit endrer seg ikke.
        let same_work_tree = commit.is_none() && self.loaded_commit.is_none();
        let touched = if same_work_tree && !paths.is_empty() {
            jetdiff_changes::touched(&self.changes, &changes, paths)
        } else {
            0
        };
        let selected = self.shown.as_ref().map(|(c, _)| c.path.clone());
        self.repo = repo;
        self.loaded_commit = commit;
        if *changes != *self.changes {
            let changes: Rc<[FileChange]> = changes.into();
            self.changes = changes.clone();
            self.tree.update(cx, |t, cx| t.set_changes(changes, cx));
        }
        // Valgt fil beholdes så lenge den er med
        let file = selected
            .and_then(|p| self.changes.iter().position(|c| c.path == p))
            .or_else(|| self.tree.read(cx).first());
        self.selection.update(cx, |s, cx| s.set(file, cx));
        if touched > 0 {
            let text = match touched {
                1 => "Oppdatert: 1 fil endret".to_owned(),
                n => format!("Oppdatert: {n} filer endret"),
            };
            self.message
                .update(cx, |m, cx| m.show(text, Level::Info, cx));
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(change) = self.selection.read(cx).0.map(|i| self.changes[i].clone()) else {
            self.shown = None;
            self.message
                .update(cx, |m, cx| m.pin("Ingen endringer", Level::Info, cx));
            self.diff
                .update(cx, |v, cx| v.set_diff(empty_diff(), Default::default(), cx));
            return;
        };
        let content = self.repo.read(&change);
        self.show(change, content, cx);
    }

    /// Er det samme fil som før, med nytt innhold, beholdes cursor og scroll.
    fn show(
        &mut self,
        change: FileChange,
        content: Result<[String; 2], String>,
        cx: &mut Context<Self>,
    ) {
        let shown = Some((change.clone(), content.clone()));
        if shown == self.shown {
            return;
        }
        let same_file = self
            .shown
            .as_ref()
            .is_some_and(|(c, _)| c.path == change.path);
        self.shown = shown;
        let (diff, notice) = build_diff(&change, content);
        self.message.update(cx, |m, cx| match notice {
            Some(text) => m.pin(text, Level::Warning, cx),
            None => m.unpin(cx),
        });
        let paths = [change.left_path().to_owned(), change.path.clone()];
        // Bare working tree kan skrives til, ikke en commit
        let can_rollback = self.repo.is_work_tree();
        self.diff.update(cx, |v, cx| {
            v.set_can_rollback(can_rollback);
            if same_file {
                v.update_diff(diff, paths, cx)
            } else {
                v.set_diff(diff, paths, cx)
            }
        });
        cx.notify();
    }

    /// Treet og commit-lista, bare de som er åpne.
    fn open_panes(&self, cx: &App) -> (Option<FocusHandle>, Option<FocusHandle>) {
        if !self.panel_open {
            return (None, None);
        }
        let tree = self.files_open.then(|| self.tree.focus_handle(cx));
        let commits = self.commits.as_ref().filter(|_| self.commits_open);
        (tree, commits.map(|c| c.focus_handle(cx)))
    }

    fn toggle_side_panel(
        &mut self,
        _: &ToggleSidePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_open = !self.panel_open;
        if !self.panel_open {
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

    /// Overskrift med + eller −. Klikk klapper seksjonen sammen eller opp.
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
            .text_color(self.theme.line_number)
            .cursor_pointer()
            .child(title)
            .child(if open { "−" } else { "+" })
            .on_click(cx.listener(move |this, _, window, cx| {
                let open = toggle(this);
                *open = !*open;
                // Fokus i en lukket seksjon går til diffen
                let tree = this.tree.focus_handle(cx);
                let commits = this.commits.as_ref().map(|c| c.focus_handle(cx));
                let hidden = (!this.files_open && tree.is_focused(window))
                    || (!this.commits_open && commits.is_some_and(|c| c.is_focused(window)));
                if hidden {
                    window.focus(&this.diff.focus_handle(cx));
                }
                cx.notify();
            }))
    }

    /// Treet med commitene under. Bredden følger treet.
    fn side_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let viewport = window.viewport_size();
        let unscaled = |v: gpui::Pixels| f32::from(v) / self.scale;
        let content = self.tree.update(cx, |t, _| t.content_width(window));
        let width = self.width.resolve(content, unscaled(viewport.width));
        let mut panel = div()
            .flex_none()
            .w(px(width * self.scale))
            .h_full()
            .flex()
            .flex_col()
            .bg(self.theme.editor_bg)
            .border_r_1()
            .border_color(self.theme.indent_guide)
            .font_family(self.font_family)
            .text_size(px(TEXT_SIZE * self.scale))
            .child(self.heading("Filer", self.files_open, cx, |this| &mut this.files_open))
            .when(self.files_open, |d| {
                d.child(div().flex_1().min_h_0().child(self.tree.clone()))
            });
        if let Some(commits) = self.commits.clone() {
            let section = div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(self.theme.indent_guide)
                .child(self.heading("Commits", self.commits_open, cx, |this| {
                    &mut this.commits_open
                }))
                .when(self.commits_open, |d| {
                    d.child(div().flex_1().min_h_0().child(commits))
                });
            // Lukket legger seksjonen seg nederst. Uten treet tar commitene resten.
            panel = match (self.files_open, self.commits_open) {
                (true, true) => {
                    let height = self.commits_height.resolve(unscaled(viewport.height));
                    PanelHeight::attach(panel, section.h(px(height * self.scale)), cx, |this| {
                        (&mut this.commits_height, this.scale)
                    })
                }
                (false, true) => panel.child(section.flex_1().min_h_0()),
                (true, false) => panel.child(section),
                (false, false) => panel.child(div().flex_1()).child(section),
            };
        }
        PanelWidth::attach(panel, cx, |this| (&mut this.width, this.scale))
    }
}

impl Focusable for ChangesView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.diff.focus_handle(cx)
    }
}

impl Render for ChangesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::toggle_side_panel))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| {
                this.tree.update(cx, |t, cx| t.step(true, cx))
            }))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| {
                this.tree.update(cx, |t, cx| t.step(false, cx))
            }))
            // Zoom og hjelpen virker også når treet eller commitene har fokus
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
            .when(self.panel_open, |d| d.child(self.side_panel(window, cx)))
            .child(div().flex_1().min_w_0().h_full().child(self.diff.clone()))
    }
}

/// Diffen for en fil, eller en tom diff og hvorfor den ikke kan vises.
fn build_diff(
    change: &FileChange,
    content: Result<[String; 2], String>,
) -> (FileDiff, Option<String>) {
    match content {
        Err(e) => (empty_diff(), Some(e)),
        Ok([l, r]) if l.contains('\0') || r.contains('\0') => {
            (empty_diff(), Some("Binærfil kan ikke vises".to_owned()))
        }
        Ok([l, r]) => match FileDiff::new(&l, &r, ComparisonPolicy::Default) {
            Ok(mut diff) => {
                let lang = Language::from_path(&change.path)
                    .or_else(|| Language::from_path(change.left_path()));
                if let Some(lang) = lang {
                    diff.highlight_syntax(lang);
                }
                (diff, None)
            }
            Err(_) => (
                empty_diff(),
                Some("Filen er for stor til å vises".to_owned()),
            ),
        },
    }
}

fn empty_diff() -> FileDiff {
    FileDiff::new("", "", ComparisonPolicy::Default).expect("tom diff er aldri for stor")
}
