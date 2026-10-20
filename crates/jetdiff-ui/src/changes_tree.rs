//! Filtreet til venstre, som i commit-dialogen til IntelliJ. Klikk på en fil velger den,
//! klikk på en mappe klapper den sammen.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Hsla, ImageSource, KeyDownEvent, Render,
    Resource, ScrollStrategy, Subscription, UniformListScrollHandle, Window, div, img, prelude::*,
    px, uniform_list,
};
use jetdiff_app::{FileTree, count_label, file_name};
use jetdiff_changes::{ChangeTree, FileChange, RowKind, Status};
use jetdiff_viewmodel::columns::display_width;

use crate::icons::{chevron_icon, file_icon, folder_icon};
use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, ROW_HEIGHT, TEXT_SIZE, list_key, row_bg};
use crate::settings::Settings;
use crate::text::FAMILY;
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
    tree: FileTree,
    selection: Entity<Selection>,
    scroll: UniformListScrollHandle,
    /// Følger zoomen i diffen, som hele UI-et i IntelliJ.
    scale: f32,
    /// Bredden innholdet trenger, uskalert. Regnes ut på nytt når endringene byttes.
    content_width: Option<f32>,
    focus: FocusHandle,
    _selection: Subscription,
}

impl ChangesTree {
    pub fn new(
        changes: Rc<[FileChange]>,
        selection: Entity<Selection>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&selection, |this, _, cx| this.reveal(cx));
        Self {
            tree: FileTree::new(changes),
            selection,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            content_width: None,
            focus: cx.focus_handle(),
            _selection: observe,
        }
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    pub fn set_changes(&mut self, changes: Rc<[FileChange]>, cx: &mut Context<Self>) {
        self.tree.set_changes(changes);
        self.content_width = None;
        cx.notify();
    }

    pub fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        if let Some(next) = self.tree.next_file(file, forward) {
            self.selection.update(cx, |s, cx| s.set(Some(next), cx));
        }
    }

    /// Valgt fil blir synlig, også når den er valgt med tastaturet i en lukket mappe.
    fn reveal(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        if let Some(row) = self.tree.reveal(file) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        cx.notify();
    }

    /// Mapper klappes sammen eller opp, filer velges.
    fn open(&mut self, file: Option<usize>, cx: &mut Context<Self>) {
        match file {
            Some(file) => self.selection.update(cx, |s, cx| s.set(Some(file), cx)),
            None => cx.notify(),
        }
    }

    /// Bredden innholdet trenger, uskalert.
    pub fn content_width(&mut self, window: &Window) -> f32 {
        *self.content_width.get_or_insert_with(|| {
            let char_width = TextStyle::new(TEXT_SIZE, window).char_width;
            content_width(self.tree.changes(), char_width)
        })
    }

    fn status_color(theme: &UiTheme, status: Status) -> Hsla {
        match status {
            Status::Added => theme.file_added,
            Status::Deleted => theme.file_deleted,
            Status::Modified | Status::Renamed => theme.file_modified,
        }
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = list_key(ev) else {
            return;
        };
        cx.stop_propagation();
        let row = match key {
            ListKey::Down => self.tree.move_cursor(true),
            ListKey::Up => self.tree.move_cursor(false),
            ListKey::Open if !self.tree.rows().is_empty() => {
                let file = self.tree.open_cursor();
                return self.open(file, cx);
            }
            ListKey::Open => return,
        };
        self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        cx.notify();
    }

    fn render_row(
        &self,
        i: usize,
        selected: Option<usize>,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let row = &self.tree.rows()[i];
        let theme = cx.global::<Settings>().theme;
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
                    &theme,
                    i == self.tree.cursor(),
                    selected.is_some_and(|f| row.kind == RowKind::File(f)),
                    focused,
                ),
                |d, bg| d.bg(bg),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let file = this.tree.open(i);
                this.open(file, cx)
            }));
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
                let change = &self.tree.changes()[f];
                let name = file_name(&change.path);
                // Plassen der mappene har pila, så ikonene står under hverandre
                base.child(div().w(s(ICON_SIZE)).flex_none())
                    .child(icon(file_icon(&change.path, dark)))
                    .child(
                        div()
                            .text_color(Self::status_color(&theme, change.status))
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
            .font_family(FAMILY)
            .text_size(px(TEXT_SIZE * self.scale))
            .text_color(cx.global::<Settings>().theme.editor_fg)
            .child(
                uniform_list(
                    "changes",
                    self.tree.rows().len(),
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
