//! Filtre og diff side om side. Henter innholdet fra git når en fil velges.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Render, Subscription, Window, div, prelude::*,
};
use smudiff_changes::FileChange;
use smudiff_git::Repo;
use smudiff_view::{ComparisonPolicy, FileDiff, Language};

use crate::changes_tree::{ChangesTree, Selection};
use crate::file_diff::{FileDiffView, StepFile, Zoomed};
use crate::message::{Level, Message};
use crate::theme::UiTheme;

pub struct ChangesView {
    repo: Repo,
    changes: Rc<[FileChange]>,
    selection: Entity<Selection>,
    tree: Entity<ChangesTree>,
    diff: Entity<FileDiffView>,
    message: Entity<Message>,
    _subscriptions: [Subscription; 3],
}

impl ChangesView {
    /// `changes` kan ikke være tom.
    pub fn new(
        repo: Repo,
        changes: Vec<FileChange>,
        theme: UiTheme,
        family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let changes: Rc<[FileChange]> = changes.into();
        let first = smudiff_changes::ChangeTree::new(&changes)
            .first()
            .unwrap_or(0);
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
        let mut view = Self {
            repo,
            changes,
            selection,
            tree,
            diff,
            message,
            _subscriptions: subscriptions,
        };
        view.load(cx);
        view
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let change = &self.changes[self.selection.read(cx).0];
        let (diff, notice) = match self.repo.read(change) {
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
