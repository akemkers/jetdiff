//! The first page: what the change does, what a reviewer needs to know first, what they accept by
//! taking it, and the parts with their steps. A click on a note or a step goes to it.

use gpui::{Context, Div, FontWeight, div, prelude::*, px, relative};
use jetdiff_app::walkthrough::Reading;

use super::WalkthroughMode;
use super::page::{chip, note_kind, wrapping};
use crate::theme::UiTheme;

const GAP: f32 = 12.;
const TITLE_SIZE: f32 = 19.;
const SECTION_SIZE: f32 = 11.;
const TEXT_SIZE: f32 = 14.;
const SMALL_SIZE: f32 = 13.;
/// About 100 characters at the text size, as on the other pages.
const TEXT_WIDTH: f32 = 840.;

impl WalkthroughMode {
    pub(super) fn overview(
        &self,
        reading: &Reading,
        theme: &UiTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = |v: f32| px(v * self.scale);
        let walkthrough = reading.walkthrough();
        let section = |title: &'static str| {
            div()
                .pt(s(GAP))
                .text_size(s(SECTION_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.line_number)
                .child(title)
        };
        let mut column = div()
            .max_w(s(TEXT_WIDTH))
            .flex()
            .flex_col()
            .gap(s(GAP))
            .text_size(s(TEXT_SIZE))
            .line_height(relative(1.5))
            .child(section("OVERVIEW"))
            .child(
                div()
                    .text_size(s(TITLE_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(walkthrough.title.clone()),
            )
            .child(wrapping(walkthrough.summary.clone()));
        if !walkthrough.before_you_review.is_empty() {
            column = column.child(section("BEFORE YOU REVIEW")).children(
                walkthrough.before_you_review.iter().map(|point| {
                    div()
                        .flex()
                        .gap(s(GAP))
                        .child("•")
                        .child(wrapping(point.clone()).flex_1().min_w_0())
                }),
            );
        }
        let notes = reading.notes();
        if !notes.is_empty() {
            column = column.child(section("WHAT YOU TAKE ON"));
            for (i, (page, note)) in notes.into_iter().enumerate() {
                column = column.child(
                    div()
                        .id(("overview-note", i))
                        .flex()
                        .items_start()
                        .gap(s(GAP))
                        .cursor_pointer()
                        .text_size(s(SMALL_SIZE))
                        .child(chip(
                            note_kind(note.kind),
                            theme.notification_warning_background,
                            theme.notification_foreground,
                            self.scale,
                        ))
                        .child(
                            wrapping(format!("{} (step {page})", note.text))
                                .flex_1()
                                .min_w_0(),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx))),
                );
            }
        }
        column = column.child(section("STEPS"));
        let mut page = 1;
        for part in &walkthrough.parts {
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(part.title.clone()),
                    )
                    .when(!part.blurb.is_empty(), |d| {
                        d.child(wrapping(part.blurb.clone()).text_color(theme.line_number))
                    }),
            );
            for step in &part.steps {
                let step_page = page;
                column = column.child(
                    div()
                        .id(("overview-step", step_page))
                        .flex()
                        .gap(s(GAP))
                        .cursor_pointer()
                        .text_size(s(SMALL_SIZE))
                        .child(
                            div()
                                .flex_none()
                                .text_color(theme.line_number)
                                .child(step_page.to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(step.title.clone()),
                        )
                        .when(step.is_key(), |d| {
                            d.child(chip(
                                "Key change",
                                theme.tree_selection,
                                theme.editor_fg,
                                self.scale,
                            ))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| this.go_to(step_page, cx))),
                );
                page += 1;
            }
        }
        column
    }
}
