//! The file tree on the left, as in IntelliJ's commit dialog. A click on a file selects it,
//! a click on a folder opens or closes it.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Hsla, ImageSource, KeyDownEvent, Render,
    Resource, ScrollStrategy, Subscription, TextRun, UniformListScrollHandle, Window, div, font,
    img, prelude::*, px, uniform_list,
};
use jetdiff_app::{Files, file_name};
use jetdiff_files::{ChangedFile, FileStatus, RowKind, Tree};

use crate::icons::{chevron_icon, file_icon, folder_icon};
use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, list_key, selection_and_cursor};
use crate::settings::Settings;
use crate::text::SANS;
use crate::theme::UiPalette;

const INDENT: f32 = 17.;
const ICON_SIZE: f32 = 16.;
const TEXT_SIZE: f32 = 14.;
const ROW_HEIGHT: f32 = 26.;
const GUIDE_LINE_WIDTH: f32 = 1.;
/// The outline of the cursor, on both sides of the row.
const CURSOR_OUTLINE: f32 = 2.;

/// The selected file, an index in the changed files. `None` when the list is empty. The tree and RepoView
/// observe it.
pub struct Selection(pub Option<usize>);

impl Selection {
    pub fn set(&mut self, file: Option<usize>, cx: &mut Context<Self>) {
        self.0 = file;
        cx.notify();
    }
}

pub struct FileTree {
    files: Files,
    selection: Entity<Selection>,
    scroll: UniformListScrollHandle,
    /// Follows the zoom in the diff, like the whole UI in IntelliJ.
    scale: f32,
    /// The width the content needs, unscaled. Recomputed when the changed files are replaced.
    content_width: Option<f32>,
    focus: FocusHandle,
    _selection: Subscription,
}

impl FileTree {
    pub fn new(
        changed_files: Rc<[ChangedFile]>,
        selection: Entity<Selection>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&selection, |this, _, cx| this.reveal(cx));
        Self {
            files: Files::new(changed_files),
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

    pub fn set_changed_files(&mut self, changed_files: Rc<[ChangedFile]>, cx: &mut Context<Self>) {
        self.files.set_changed_files(changed_files);
        self.content_width = None;
        cx.notify();
    }

    pub fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        if let Some(next) = self.files.next_file(file, forward) {
            self.selection.update(cx, |s, cx| s.set(Some(next), cx));
        }
    }

    /// The selected file becomes visible, also when it was selected with the keyboard in a closed folder.
    fn reveal(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.selection.read(cx).0 else {
            return;
        };
        if let Some(row) = self.files.reveal(file) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        cx.notify();
    }

    /// `None` after a folder row, which opened or closed instead.
    fn selected(&mut self, file: Option<usize>, cx: &mut Context<Self>) {
        match file {
            Some(file) => self.selection.update(cx, |s, cx| s.set(Some(file), cx)),
            None => cx.notify(),
        }
    }

    /// The width the content needs, unscaled, measured in the font the rows use.
    pub fn content_width(&mut self, window: &Window) -> f32 {
        *self.content_width.get_or_insert_with(|| {
            let measure = |text: &str| {
                let run = TextRun {
                    len: text.len(),
                    font: font(SANS),
                    color: Hsla::default(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line = window
                    .text_system()
                    .layout_line(text, px(TEXT_SIZE), &[run], None);
                f32::from(line.width)
            };
            content_width(self.files.changed_files(), measure)
        })
    }

    fn status_color(palette: &UiPalette, status: FileStatus) -> Hsla {
        match status {
            FileStatus::Added => palette.added_pill.text,
            FileStatus::Deleted => palette.deleted_pill.text,
            FileStatus::Modified | FileStatus::Renamed => palette.primary,
        }
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = list_key(ev) else {
            return;
        };
        cx.stop_propagation();
        let row = match key {
            ListKey::Down => self.files.move_cursor(true),
            ListKey::Up => self.files.move_cursor(false),
            ListKey::Select if !self.files.rows().is_empty() => {
                let file = self.files.select_at_cursor();
                return self.selected(file, cx);
            }
            ListKey::Select => return,
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
        let row = &self.files.rows()[i];
        let palette = cx.global::<Settings>().theme.palette;
        let s = |v: f32| px(v * self.scale);
        // Explicitly Embedded, otherwise GPUI reads `java.svg` as a URI and fetches it over HTTP
        let icon = |path: String| {
            img(ImageSource::Resource(Resource::Embedded(path.into())))
                .size(s(ICON_SIZE))
                .flex_none()
        };
        let is_selected = selected.is_some_and(|file| row.kind == RowKind::File(file));
        let guide_lines = (0..row.depth).map(|level| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(s(PAD_LEFT
                    + level as f32 * INDENT
                    + (ICON_SIZE - GUIDE_LINE_WIDTH) / 2.))
                .w(s(GUIDE_LINE_WIDTH))
                .bg(palette.tree_guide_line)
        });
        let base = div()
            .id(i)
            .relative()
            .h(s(ROW_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap(s(GAP))
            .pl(s(PAD_LEFT + row.depth as f32 * INDENT))
            .pr(s(PAD_RIGHT))
            .whitespace_nowrap()
            .overflow_hidden()
            .children(guide_lines)
            .on_click(cx.listener(move |this, _, _, cx| {
                let file = this.files.select(i);
                this.selected(file, cx)
            }));
        let base = selection_and_cursor(
            base,
            is_selected,
            i == self.files.cursor(),
            focused,
            &palette,
        );
        match &row.kind {
            RowKind::Dir {
                name, count, open, ..
            } => base
                .text_color(palette.secondary)
                .child(icon(chevron_icon(*open)))
                .child(icon(folder_icon()))
                .child(name.clone())
                .child(div().flex_1())
                .child(div().text_color(palette.muted).child(count.to_string())),
            &RowKind::File(file) => {
                let file = &self.files.changed_files()[file];
                let name_color = if is_selected {
                    palette.strong
                } else {
                    Self::status_color(&palette, file.status)
                };
                base.child(icon(file_icon(&file.path)))
                    .child(
                        div()
                            .text_color(name_color)
                            .child(file_name(&file.path).to_owned()),
                    )
                    .when_some(file.renamed_from_label(), |row, from| {
                        row.child(div().text_color(palette.muted).child(from))
                    })
            }
        }
    }
}

impl Focusable for FileTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .font_family(SANS)
            .text_size(px(TEXT_SIZE * self.scale))
            .text_color(cx.global::<Settings>().theme.palette.primary)
            .child(
                uniform_list(
                    "files",
                    self.files.rows().len(),
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

/// The width that fits all rows with all folders open, unscaled.
fn content_width(changed_files: &[ChangedFile], measure: impl Fn(&str) -> f32) -> f32 {
    Tree::new(changed_files)
        .rows()
        .iter()
        .map(|row| {
            let content = match &row.kind {
                RowKind::Dir { name, count, .. } => {
                    2. * (ICON_SIZE + GAP) + measure(name) + GAP + measure(&count.to_string())
                }
                &RowKind::File(file) => {
                    let file = &changed_files[file];
                    let from = file
                        .renamed_from_label()
                        .map_or(0., |label| GAP + measure(&label));
                    ICON_SIZE + GAP + measure(file_name(&file.path)) + from
                }
            };
            PAD_LEFT + row.depth as f32 * INDENT + content
        })
        .fold(0., f32::max)
        + PAD_RIGHT
        + CURSOR_OUTLINE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_follows_longest_row() {
        let changed_file = |path: &str| ChangedFile {
            path: path.into(),
            renamed_from: None,
            status: FileStatus::Modified,
        };
        let measure = |text: &str| text.chars().count() as f32 * 10.;
        let edges = PAD_LEFT + PAD_RIGHT + CURSOR_OUTLINE;
        // The folder `source` with its count 1 is wider than `a.rs` one level in
        let folder = 2. * (ICON_SIZE + GAP) + 60. + GAP + 10.;
        assert_eq!(
            content_width(&[changed_file("source/a.rs")], measure),
            edges + folder
        );
        let long = format!("src/{}.rs", "x".repeat(17));
        let file = INDENT + ICON_SIZE + GAP + 200.;
        assert_eq!(content_width(&[changed_file(&long)], measure), edges + file);
    }
}
