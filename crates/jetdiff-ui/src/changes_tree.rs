//! Filtreet til venstre, som i commit-dialogen til IntelliJ. Klikk på en fil velger den,
//! klikk på en mappe klapper den sammen.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Hsla, ImageSource, KeyDownEvent, Render,
    Resource, ScrollStrategy, Subscription, UniformListScrollHandle, Window, div, img, prelude::*,
    px, uniform_list,
};
use jetdiff_changes::{ChangeTree, FileChange, Row, RowKind, Status};
use jetdiff_view::columns::display_width;

use crate::icons::{chevron_icon, file_icon, folder_icon};
use crate::list::{
    GAP, ListKey, PAD_LEFT, PAD_RIGHT, ROW_HEIGHT, TEXT_SIZE, list_key, row_bg, step,
};
use crate::text::TextStyle;
use crate::theme::UiTheme;

const INDENT: f32 = 16.;
const ICON_SIZE: f32 = 16.;

/// Valgt fil, indeks i endringslista. `None` når lista er tom. Treet og ChangesView
/// observerer den.
pub struct Selection(pub Option<usize>);

impl Selection {
    pub fn set(&mut self, file: Option<usize>, cx: &mut Context<Self>) {
        self.0 = file;
        cx.notify();
    }
}

pub struct ChangesTree {
    changes: Rc<[FileChange]>,
    tree: ChangeTree,
    rows: Rc<[Row]>,
    selection: Entity<Selection>,
    theme: UiTheme,
    font_family: &'static str,
    scroll: UniformListScrollHandle,
    /// Følger zoomen i diffen, som hele UI-et i IntelliJ.
    scale: f32,
    /// Bredden innholdet trenger, uskalert. Regnes ut på nytt når endringene byttes.
    content_width: Option<f32>,
    /// Raden j og k flytter. Enter åpner den.
    cursor: usize,
    focus: FocusHandle,
    _selection: Subscription,
}

impl ChangesTree {
    pub fn new(
        changes: Rc<[FileChange]>,
        selection: Entity<Selection>,
        theme: UiTheme,
        font_family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree = ChangeTree::new(&changes);
        let observe = cx.observe(&selection, |this, _, cx| this.reveal(cx));
        Self {
            rows: tree.rows().into(),
            changes,
            tree,
            selection,
            theme,
            font_family,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            content_width: None,
            cursor: 0,
            focus: cx.focus_handle(),
            _selection: observe,
        }
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    pub fn set_changes(&mut self, changes: Rc<[FileChange]>, cx: &mut Context<Self>) {
        self.tree.rebuild(&changes);
        self.rows = self.tree.rows().into();
        self.changes = changes;
        self.content_width = None;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    pub fn first(&self) -> Option<usize> {
        self.tree.first()
    }

    pub fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        let next = self.tree.step(file, forward);
        if next != file {
            self.selection.update(cx, |s, cx| s.set(Some(next), cx));
        }
    }

    /// Valgt fil blir synlig, også når den er valgt med tastaturet i en lukket mappe.
    fn reveal(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        self.tree.reveal(file, &self.changes);
        self.rows = self.tree.rows().into();
        if let Some(i) = self.rows.iter().position(|r| r.kind == RowKind::File(file)) {
            self.cursor = i;
            self.scroll.scroll_to_item(i, ScrollStrategy::Top);
        }
        cx.notify();
    }

    /// Mapper klappes sammen eller opp, filer velges.
    fn open(&mut self, row: usize, cx: &mut Context<Self>) {
        self.cursor = row;
        match &self.rows[row].kind {
            RowKind::Dir { path, .. } => {
                self.tree.toggle(&path.clone());
                self.rows = self.tree.rows().into();
                cx.notify();
            }
            &RowKind::File(file) => self.selection.update(cx, |s, cx| s.set(Some(file), cx)),
        }
    }

    /// Bredden innholdet trenger, uskalert.
    pub fn content_width(&mut self, window: &Window) -> f32 {
        *self.content_width.get_or_insert_with(|| {
            let char_width = TextStyle::new(self.font_family, TEXT_SIZE, window).char_width;
            content_width(&self.changes, char_width)
        })
    }

    fn status_color(&self, status: Status) -> Hsla {
        match status {
            Status::Added => self.theme.file_added,
            Status::Deleted => self.theme.file_deleted,
            Status::Modified | Status::Renamed => self.theme.file_modified,
        }
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = list_key(ev) else {
            return;
        };
        cx.stop_propagation();
        match key {
            ListKey::Down | ListKey::Up => {
                self.cursor = step(self.cursor, matches!(key, ListKey::Down), self.rows.len())
            }
            ListKey::Open if !self.rows.is_empty() => return self.open(self.cursor, cx),
            ListKey::Open => return,
        }
        self.scroll.scroll_to_item(self.cursor, ScrollStrategy::Top);
        cx.notify();
    }

    fn render_row(
        &self,
        i: usize,
        selected: Option<usize>,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let row = &self.rows[i];
        let theme = &self.theme;
        let dark = theme.dark;
        let s = |v: f32| px(v * self.scale);
        // Eksplisitt Embedded, ellers tolker GPUI `java.svg` som en URI og henter over HTTP
        let icon = |path: String| {
            img(ImageSource::Resource(Resource::Embedded(path.into())))
                .size(s(ICON_SIZE))
                .flex_none()
        };
        let base = div()
            .id(i)
            .h(s(ROW_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap(s(GAP))
            .pl(s(PAD_LEFT + row.depth as f32 * INDENT))
            .whitespace_nowrap()
            .overflow_hidden()
            .when_some(
                row_bg(
                    theme,
                    i == self.cursor,
                    selected.is_some_and(|f| row.kind == RowKind::File(f)),
                    focused,
                ),
                |d, bg| d.bg(bg),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open(i, cx)));
        match &row.kind {
            RowKind::Dir {
                name,
                count,
                expanded,
                ..
            } => base
                .child(icon(chevron_icon(*expanded, dark)))
                .child(icon(folder_icon(dark)))
                .child(name.clone())
                .child(
                    div()
                        .pl(s(2.))
                        .text_color(theme.line_number)
                        .child(count_label(*count)),
                ),
            &RowKind::File(f) => {
                let change = &self.changes[f];
                let name = file_name(&change.path);
                // Plassen der mappene har pila, så ikonene står under hverandre
                base.child(div().w(s(ICON_SIZE)).flex_none())
                    .child(icon(file_icon(&change.path, dark)))
                    .child(
                        div()
                            .text_color(self.status_color(change.status))
                            .child(name.to_owned()),
                    )
                    .when_some(change.renamed_from(), |d, from| {
                        d.child(div().pl(s(2.)).text_color(theme.line_number).child(from))
                    })
            }
        }
    }
}

impl Focusable for ChangesTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ChangesTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .font_family(self.font_family)
            .text_size(px(TEXT_SIZE * self.scale))
            .text_color(self.theme.editor_fg)
            .child(
                uniform_list(
                    "changes",
                    self.rows.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        let selected = this.selection.read(cx).0;
                        range
                            .map(|i| this.render_row(i, selected, focused, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .track_scroll(self.scroll.clone()),
            )
    }
}

fn count_label(count: usize) -> String {
    let files = if count == 1 { "fil" } else { "filer" };
    format!("{count} {files}")
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or_default()
}

/// Bredden som får plass til alle rader med alle mapper åpne, uskalert. Fonten er monospace.
fn content_width(changes: &[FileChange], char_width: f32) -> f32 {
    let text = |s: &str| display_width(s) as f32 * char_width;
    let icons = 2. * (ICON_SIZE + GAP);
    ChangeTree::new(changes)
        .rows()
        .iter()
        .map(|r| {
            let row = match &r.kind {
                RowKind::Dir { name, count, .. } => {
                    icons + text(name) + GAP + 2. + text(&count_label(*count))
                }
                &RowKind::File(f) => {
                    let change = &changes[f];
                    let from = change.renamed_from().map_or(0., |t| GAP + 2. + text(&t));
                    icons + text(file_name(&change.path)) + from
                }
            };
            PAD_LEFT + r.depth as f32 * INDENT + row
        })
        .fold(0., f32::max)
        + PAD_RIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bredde_etter_lengste_rad() {
        let change = |path: &str| FileChange {
            path: path.into(),
            old_path: None,
            status: Status::Modified,
        };
        // Mappa `src` med «1 fil» er bredere enn `a.rs` ett nivå inn
        let dir = PAD_LEFT + 2. * (ICON_SIZE + GAP) + 30. + GAP + 2. + 50.;
        assert_eq!(content_width(&[change("src/a.rs")], 10.), dir + PAD_RIGHT);
        let file = PAD_LEFT + INDENT + 2. * (ICON_SIZE + GAP) + 200.;
        let long = format!("src/{}.rs", "x".repeat(17));
        assert_eq!(content_width(&[change(&long)], 10.), file + PAD_RIGHT);
    }
}
