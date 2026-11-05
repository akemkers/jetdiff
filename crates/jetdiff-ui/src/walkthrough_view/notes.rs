//! Notes as a table, with the kind in a column of its own, so a decision stands apart from upkeep at
//! a glance. On the overview each row goes to its step.

use gpui::{Context, Div, FontWeight, div, font, prelude::*, px, relative};
use jetdiff_app::walkthrough::{Note, NoteKind};

use super::WalkthroughView;
use super::page::wrapping;
use super::prose::{ProseColors, SANS, prose};
use crate::settings::Settings;
use crate::theme::UiPalette;

const NOTE_SIZE: f32 = 15.;
const LINK_SIZE: f32 = 13.;
const TAG_SIZE: f32 = 12.;
const TAG_HEIGHT: f32 = 22.;
const TAG_COLUMN: f32 = 112.;
const GAP: f32 = 16.;
const ROW_PADDING: (f32, f32) = (16., 18.);

pub(super) struct NoteRow<'a> {
    pub note: &'a Note,
    /// The step a click goes to. `None` on the step itself.
    pub page: Option<usize>,
}

impl WalkthroughView {
    pub(super) fn notes_table(&self, rows: Vec<NoteRow>, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let palette = cx.global::<Settings>().theme.palette;
        let body = ProseColors::body(&palette);
        let (vertical, horizontal) = ROW_PADDING;
        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(i, NoteRow { note, page })| {
                div()
                    .id(("note", i))
                    .px(s(horizontal))
                    .py(s(vertical))
                    .flex()
                    .items_start()
                    .gap(s(GAP))
                    .when(i > 0, |d| d.border_t_1().border_color(palette.border))
                    .text_size(s(NOTE_SIZE))
                    .line_height(relative(1.6))
                    .child(
                        div()
                            .flex_none()
                            .w(s(TAG_COLUMN))
                            .flex()
                            .child(note_tag(note.kind, &palette, self.scale)),
                    )
                    .child(wrapping(prose(&note.text, &body)).flex_1().min_w_0())
                    .when_some(page, |d, page| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(palette.selected))
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(s(LINK_SIZE))
                                    .text_color(palette.accent)
                                    .child(format!("Step {page} →")),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx)))
                    })
            });
        div()
            .flex()
            .flex_col()
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .rounded(s(8.))
            .overflow_hidden()
            .font_family(SANS)
            .children(rows)
    }
}

/// A pill with the note's kind. The kinds differ in lightness, not only in hue.
pub(super) fn note_tag(kind: NoteKind, palette: &UiPalette, scale: f32) -> Div {
    let (pill, name) = match kind {
        NoteKind::Decision => (palette.decision_tag, "Decision"),
        NoteKind::Upkeep => (palette.upkeep_tag, "Upkeep"),
        NoteKind::Limitation => (palette.limitation_tag, "Limitation"),
        NoteKind::Divergence => (palette.divergence_tag, "Divergence"),
        NoteKind::Untested => (palette.untested_tag, "Untested"),
    };
    div()
        .flex_none()
        .h(px(TAG_HEIGHT * scale))
        .px(px(10. * scale))
        .flex()
        .items_center()
        .rounded_full()
        .bg(pill.background)
        .text_color(pill.text)
        .font(font(SANS))
        .text_size(px(TAG_SIZE * scale))
        .line_height(relative(1.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(name)
}
