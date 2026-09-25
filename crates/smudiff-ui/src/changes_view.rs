//! Filtre og diff side om side. Henter innholdet fra git når en fil velges, og på nytt
//! når working tree endrer seg.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Render, Subscription, Task, Window, div,
    prelude::*,
};
use smudiff_changes::FileChange;
use smudiff_git::Repo;
use smudiff_view::{ComparisonPolicy, FileDiff, Language};

use crate::changes_tree::{ChangesTree, Selection};
use crate::file_diff::{FileDiffView, StepFile, Zoomed};
use crate::message::{Level, Message};
use crate::theme::UiTheme;
use crate::watcher::Watcher;

pub struct ChangesView {
    repo: Repo,
    changes: Rc<[FileChange]>,
    selection: Entity<Selection>,
    tree: Entity<ChangesTree>,
    diff: Entity<FileDiffView>,
    message: Entity<Message>,
    /// Det diffen viser nå. Samme innhold etter en reload lar posisjonen stå.
    shown: Option<(FileChange, Result<[String; 2], String>)>,
    _watcher: Option<Watcher>,
    _reload: Option<Task<()>>,
    _subscriptions: [Subscription; 3],
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
        let subscriptions = [
            cx.observe(&selection, |this, _, cx| this.load(cx)),
            cx.subscribe(&diff, |this, _, e: &StepFile, cx| {
                this.tree.update(cx, |t, cx| t.step(e.forward, cx))
            }),
            cx.subscribe(&diff, |this, _, e: &Zoomed, cx| {
                this.tree
                    .update(cx, |t, cx| t.set_font_size(e.font_size, cx))
            }),
        ];
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
            changes,
            selection,
            tree,
            diff,
            message,
            shown: None,
            _watcher: watcher,
            _reload: None,
            _subscriptions: subscriptions,
        };
        view.load(cx);
        view
    }

    /// Git leses i bakgrunnen. En ny endring underveis avbryter den forrige.
    fn changed(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let root = self.repo.root().to_owned();
        self._reload = Some(cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let repo = Repo::work_tree(&root)?;
                    let changes = repo.changes()?;
                    Ok((repo, changes))
                })
                .await;
            this.update(cx, |v, cx| v.reloaded(loaded, &paths, cx)).ok();
        }));
    }

    fn reloaded(
        &mut self,
        loaded: Result<(Repo, Vec<FileChange>), String>,
        paths: &[String],
        cx: &mut Context<Self>,
    ) {
        let (repo, changes) = match loaded {
            Ok(l) => l,
            Err(e) => {
                self.message
                    .update(cx, |m, cx| m.show(e, Level::Warning, cx));
                return;
            }
        };
        let touched = smudiff_changes::touched(&self.changes, &changes, paths);
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

impl Render for ChangesView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(self.tree.clone())
            .child(div().flex_1().min_w_0().h_full().child(self.diff.clone()))
    }
}
