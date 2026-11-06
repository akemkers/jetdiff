//! The row above the diff: the path and the counts of added and deleted lines at the left, the
//! file controls at the right.

use gpui::{Context, Div, FontWeight, div, prelude::*, px};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::theme::Pill;

use super::FileDiffView;
use crate::settings::Settings;
use crate::text::{FAMILY, SANS};
use crate::theme::UiPalette;

const HORIZONTAL_PADDING: f32 = 20.;
const VERTICAL_PADDING: f32 = 10.;
const GAP: f32 = 10.;
const PATH_SIZE: f32 = 14.;
const PILL_SIZE: f32 = 12.;
const PILL_PADDING: f32 = 6.;
const PILL_RADIUS: f32 = 4.;

impl FileDiffView {
    /// `None` when no file is shown.
    pub(super) fn diff_header(&self, scale: f32, cx: &mut Context<Self>) -> Option<Div> {
        let path = &self.paths[Side::Right.select(0, 1)];
        if path.is_empty() {
            return None;
        }
        let palette = cx.global::<Settings>().theme.palette;
        let s = |v: f32| px(v * scale);
        let diff = self.mode.diff();
        let added = diff.changed_line_count(Side::Right);
        let deleted = diff.changed_line_count(Side::Left);
        Some(
            div()
                .flex_none()
                .w_full()
                .px(s(HORIZONTAL_PADDING))
                .py(s(VERTICAL_PADDING))
                .flex()
                .items_center()
                .gap(s(GAP))
                .overflow_hidden()
                .bg(palette.page)
                .border_b_1()
                .border_color(palette.border)
                .child(path_label(path, &palette, scale))
                .child(pill(format!("+{added}"), palette.added_pill, scale))
                .child(pill(format!("−{deleted}"), palette.deleted_pill, scale))
                .child(div().flex_1())
                .child(self.file_controls(&palette, scale, cx)),
        )
    }
}

/// The folder gives way when space is tight, and the file name always shows.
fn path_label(path: &str, palette: &UiPalette, scale: f32) -> Div {
    let (folder, name) = match path.rsplit_once('/') {
        Some((folder, name)) => (Some(format!("{folder}/")), name),
        None => (None, path),
    };
    div()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .font_family(FAMILY)
        .text_size(px(PATH_SIZE * scale))
        .children(folder.map(|folder| {
            div()
                .min_w_0()
                .truncate()
                .text_color(palette.muted)
                .child(folder)
        }))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::BOLD)
                .text_color(palette.strong)
                .child(name.to_owned()),
        )
}

fn pill(text: String, colors: Pill<gpui::Hsla>, scale: f32) -> Div {
    div()
        .flex_none()
        .px(px(PILL_PADDING * scale))
        .rounded(px(PILL_RADIUS * scale))
        .bg(colors.background)
        .text_color(colors.text)
        .font_family(SANS)
        .text_size(px(PILL_SIZE * scale))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}
