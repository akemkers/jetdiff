//! Filtreet til venstre, som i commit-dialogen til IntelliJ. Klikk på en fil velger den,
//! klikk på en mappe klapper den sammen.

use std::rc::Rc;

use gpui::{
    Context, Entity, Hsla, ImageSource, Render, Resource, ScrollStrategy, Subscription,
    UniformListScrollHandle, Window, div, img, prelude::*, px, uniform_list,
};
use smudiff_changes::{ChangeTree, FileChange, Row, RowKind, Status};
use smudiff_view::metrics::FONT_SIZE;

use crate::icons::{chevron_icon, file_icon, folder_icon};
use crate::theme::UiTheme;

const TREE_WIDTH: f32 = 280.;
const ROW_HEIGHT: f32 = 22.;
const INDENT: f32 = 16.;
const ICON_SIZE: f32 = 16.;

/// Valgt fil, indeks i endringslista. Treet og diffen observerer den.
pub struct Selection(pub usize);

pub struct ChangesTree {
    changes: Rc<[FileChange]>,
    tree: ChangeTree,
    rows: Rc<[Row]>,
    selection: Entity<Selection>,
    theme: UiTheme,
    family: &'static str,
    scroll: UniformListScrollHandle,
    /// Følger zoomen i diffen, som hele UI-et i IntelliJ.
    scale: f32,
    _selection: Subscription,
}

impl ChangesTree {
    pub fn new(
        changes: Rc<[FileChange]>,
        selection: Entity<Selection>,
        theme: UiTheme,
        family: &'static str,
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
            family,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            _selection: observe,
        }
    }

    pub fn set_font_size(&mut self, font_size: f32, cx: &mut Context<Self>) {
        self.scale = font_size / FONT_SIZE;
        cx.notify();
    }

    pub fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let file = self.selection.read(cx).0;
        let next = self.tree.step(file, forward);
        if next != file {
            self.selection.update(cx, |s, cx| {
                s.0 = next;
                cx.notify();
            });
        }
    }

    /// Valgt fil blir synlig, også når den er valgt med tastaturet i en lukket mappe.
    fn reveal(&mut self, cx: &mut Context<Self>) {
        let file = self.selection.read(cx).0;
        self.tree.reveal(file, &self.changes);
        self.rows = self.tree.rows().into();
        if let Some(i) = self.rows.iter().position(|r| r.kind == RowKind::File(file)) {
            self.scroll.scroll_to_item(i, ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn click(&mut self, row: usize, cx: &mut Context<Self>) {
        match &self.rows[row].kind {
            RowKind::Dir { path, .. } => {
                self.tree.toggle(&path.clone());
                self.rows = self.tree.rows().into();
                cx.notify();
            }
            &RowKind::File(file) => self.selection.update(cx, |s, cx| {
                s.0 = file;
                cx.notify();
            }),
        }
    }

    fn status_color(&self, status: Status) -> Hsla {
        match status {
            Status::Added => self.theme.file_added,
            Status::Deleted => self.theme.file_deleted,
            Status::Modified | Status::Renamed => self.theme.file_modified,
        }
    }

    fn render_row(
        &self,
        i: usize,
        selected: usize,
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
            .gap(s(4.))
            .pl(s(8. + row.depth as f32 * INDENT))
            .whitespace_nowrap()
            .overflow_hidden()
            .when(row.kind == RowKind::File(selected), |d| {
                d.bg(theme.tree_selection)
            })
            .on_click(cx.listener(move |this, _, _, cx| this.click(i, cx)));
        match &row.kind {
            RowKind::Dir {
                name,
                count,
                expanded,
                ..
            } => {
                let files = if *count == 1 { "fil" } else { "filer" };
                base.child(icon(chevron_icon(*expanded, dark)))
                    .child(icon(folder_icon(dark)))
                    .child(name.clone())
                    .child(
                        div()
                            .pl(s(2.))
                            .text_color(theme.line_number)
                            .child(format!("{count} {files}")),
                    )
            }
            &RowKind::File(f) => {
                let change = &self.changes[f];
                let name = change.path.rsplit('/').next().unwrap_or_default();
                // Plassen der mappene har pila, så ikonene står under hverandre
                base.child(div().w(s(ICON_SIZE)).flex_none())
                    .child(icon(file_icon(&change.path, dark)))
                    .child(
                        div()
                            .text_color(self.status_color(change.status))
                            .child(name.to_owned()),
                    )
            }
        }
    }
}

impl Render for ChangesTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let s = |v: f32| px(v * self.scale);
        div()
            .flex_none()
            .w(s(TREE_WIDTH))
            .h_full()
            .bg(theme.editor_bg)
            .border_r_1()
            .border_color(theme.indent_guide)
            .font_family(self.family)
            .text_size(s(13.))
            .text_color(theme.editor_fg)
            .child(
                uniform_list(
                    "changes",
                    self.rows.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let selected = this.selection.read(cx).0;
                        range
                            .map(|i| this.render_row(i, selected, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .track_scroll(self.scroll.clone()),
            )
    }
}
