//! Filtre og diff side om side, med commitene under treet. Henter innholdet fra git når en
//! fil eller commit velges, og på nytt når working tree endrer seg.

use std::rc::Rc;

use gpui::{
    App, Context, Div, Entity, FocusHandle, Focusable, Render, Subscription, Task, Window, div,
    prelude::*, px,
};
use smudiff_changes::FileChange;
use smudiff_git::{Commit, Repo};
use smudiff_view::metrics::FONT_SIZE;
use smudiff_view::{ComparisonPolicy, FileDiff, Language};

use crate::changes_tree::{ChangesTree, Selection};
use crate::commit_list::{CommitList, SelectCommit};
use crate::file_diff::{FileDiffView, StepFile, Zoomed};
use crate::message::{Level, Message};
use crate::panel_size::{PanelHeight, PanelWidth};
use crate::theme::UiTheme;
use crate::watcher::Watcher;

/// Det som hentes fra git i bakgrunnen.
struct Loaded {
    repo: Repo,
    changes: Vec<FileChange>,
    label: String,
    commits: Vec<Commit>,
}

pub struct ChangesView {
    repo: Repo,
    /// Commiten som vises. `None` er working tree.
    commit: Option<String>,
    changes: Rc<[FileChange]>,
    selection: Entity<Selection>,
    tree: Entity<ChangesTree>,
    diff: Entity<FileDiffView>,
    message: Entity<Message>,
    /// Bare for working tree.
    commits: Option<Entity<CommitList>>,
    theme: UiTheme,
    scale: f32,
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
        family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let changes: Rc<[FileChange]> = changes.into();
        let first = smudiff_changes::ChangeTree::new(&changes).first();
        let selection = cx.new(|_| Selection(first));
        let tree =
            cx.new(|cx| ChangesTree::new(changes.clone(), selection.clone(), theme, family, cx));
        let message = cx.new(|_| Message::new(theme, family));
        let diff = cx.new(|cx| {
            FileDiffView::new(
                empty_diff(),
                Default::default(),
                message.clone(),
                theme,
                family,
                cx,
            )
            .with_file_keys()
        });
        let commits = repo.is_work_tree().then(|| {
            let list = repo.commits().unwrap_or_default();
            cx.new(|_| CommitList::new(repo.label(), list, theme, family))
        });
        let mut subscriptions = vec![
            cx.observe(&selection, |this, _, cx| this.load(cx)),
            cx.subscribe(&diff, |this, _, e: &StepFile, cx| {
                this.tree.update(cx, |t, cx| t.step(e.forward, cx))
            }),
            cx.subscribe(&diff, |this, _, e: &Zoomed, cx| {
                this.zoomed(e.font_size, cx)
            }),
        ];
        if let Some(commits) = &commits {
            subscriptions.push(cx.subscribe(commits, |this, _, e: &SelectCommit, cx| {
                this.commit = e.0.clone();
                this.changed(Vec::new(), cx);
            }));
        }
        let watcher = repo
            .is_work_tree()
            .then(|| Watcher::new(repo.root(), cx, Self::changed))
            .and_then(|w| {
                w.map_err(|e| {
                    let text = format!("Følger ikke endringer: {e}");
                    message.update(cx, |m, cx| m.show(text, Level::Warning, cx));
                })
                .ok()
            });
        let mut view = Self {
            repo,
            commit: None,
            changes,
            selection,
            tree,
            diff,
            message,
            commits,
            theme,
            scale: 1.,
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
        self.tree.update(cx, |t, cx| t.set_font_size(font_size, cx));
        if let Some(commits) = &self.commits {
            commits.update(cx, |c, cx| c.set_font_size(font_size, cx));
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
                    let repo = match commit {
                        Some(hash) => Repo::commit(&root, &hash)?,
                        None => home,
                    };
                    let changes = repo.changes()?;
                    Ok(Loaded {
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
            repo,
            changes,
            label,
            commits,
        } = match loaded {
            Ok(l) => l,
            Err(e) => {
                self.message
                    .update(cx, |m, cx| m.show(e, Level::Warning, cx));
                return;
            }
        };
        if let Some(list) = &self.commits {
            list.update(cx, |l, cx| l.set_commits(label, commits, cx));
        }
        // En commit endrer seg ikke, og et bytte er ingen oppdatering
        let touched = match self.commit {
            None if !paths.is_empty() => smudiff_changes::touched(&self.changes, &changes, paths),
            _ => 0,
        };
        let selected = self.shown.as_ref().map(|(c, _)| c.path.clone());
        self.repo = repo;
        if *changes != *self.changes {
            let changes: Rc<[FileChange]> = changes.into();
            self.changes = changes.clone();
            self.tree.update(cx, |t, cx| t.set_changes(changes, cx));
        }
        // Valgt fil beholdes så lenge den er med
        let file = selected
            .and_then(|p| self.changes.iter().position(|c| c.path == p))
            .or_else(|| self.tree.read(cx).first());
        self.selection.update(cx, |s, cx| {
            s.0 = file;
            cx.notify();
        });
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
        let shown = Some((change.clone(), content.clone()));
        if shown == self.shown {
            return;
        }
        self.shown = shown;
        let (diff, notice) = match content {
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
        };
        self.message.update(cx, |m, cx| match notice {
            Some(text) => m.pin(text, Level::Warning, cx),
            None => m.unpin(cx),
        });
        let paths = [change.left_path().to_owned(), change.path.clone()];
        self.diff.update(cx, |v, cx| v.set_diff(diff, paths, cx));
        cx.notify();
    }
}

fn empty_diff() -> FileDiff {
    FileDiff::new("", "", ComparisonPolicy::Default).expect("tom diff er aldri for stor")
}

impl Focusable for ChangesView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.diff.focus_handle(cx)
    }
}

impl ChangesView {
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
            .child(div().flex_1().min_h_0().child(self.tree.clone()));
        if let Some(commits) = self.commits.clone() {
            let height = self.commits_height.resolve(unscaled(viewport.height));
            let section = div()
                .h(px(height * self.scale))
                .border_t_1()
                .border_color(self.theme.indent_guide)
                .child(commits);
            panel = PanelHeight::attach(panel, section, cx, |this| {
                (&mut this.commits_height, this.scale)
            });
        }
        PanelWidth::attach(panel, cx, |this| (&mut this.width, this.scale))
    }
}

impl Render for ChangesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(self.side_panel(window, cx))
            .child(div().flex_1().min_w_0().h_full().child(self.diff.clone()))
    }
}
