//! The contents on the left: the overview, each part with its steps, and the files no step covers. A
//! click on a row goes to its page.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px};
use jetdiff_app::walkthrough::{ContentsRow, Reading, Weight};

use super::WalkthroughMode;
use crate::list::row_bg;
use crate::settings::Settings;

const WIDTH: f32 = 300.;
const PADDING: f32 = 14.;
const TEXT_SIZE: f32 = 13.;
const SMALL_SIZE: f32 = 11.;
const ROW_HEIGHT: f32 = 26.;
const BADGE_SIZE: f32 = 18.;
const KEY_DOT_SIZE: f32 = 6.;

impl WalkthroughMode {
    pub(super) fn contents(&self, reading: &Reading, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = |v: f32| px(v * self.scale);
        let theme = cx.global::<Settings>().theme;
        let walkthrough = reading.walkthrough();
        let mut rows = Vec::new();
        for row in reading.contents() {
            let element = match row {
                ContentsRow::Overview { shown } => self
                    .row(0, shown, cx)
                    .child(
                        div()
                            .flex_none()
                            .w(s(BADGE_SIZE))
                            .text_color(theme.line_number)
                            .child("◆"),
                    )
                    .child(div().flex_1().child("Overview"))
                    .into_any_element(),
                ContentsRow::Part(part) => div()
                    .pt(s(PADDING))
                    .pb(s(PADDING / 3.))
                    .text_size(s(SMALL_SIZE))
                    .text_color(theme.line_number)
                    .child(part.title.to_uppercase())
                    .into_any_element(),
                ContentsRow::Step {
                    page,
                    number,
                    step,
                    seen,
                    shown,
                } => {
                    let badge = if seen && !shown {
                        "✓".to_owned()
                    } else {
                        number.to_string()
                    };
                    let skim = step.weight == Weight::Skim;
                    self.row(page, shown, cx)
                        .child(
                            div()
                                .flex_none()
                                .size(s(BADGE_SIZE))
                                .rounded(s(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(theme.tree_selection_inactive)
                                .text_size(s(SMALL_SIZE))
                                .text_color(if seen {
                                    theme.file_added
                                } else {
                                    theme.line_number
                                })
                                .child(badge),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .when(skim, |d| d.text_color(theme.line_number))
                                .child(SharedString::from(step.title.clone())),
                        )
                        .when(step.is_key(), |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .size(s(KEY_DOT_SIZE))
                                    .rounded_full()
                                    .bg(theme.file_modified),
                            )
                        })
                        .into_any_element()
                }
                ContentsRow::NotCovered { page, files, shown } => self
                    .row(page, shown, cx)
                    .mt(s(PADDING / 2.))
                    .border_t_1()
                    .border_color(theme.indent_guide)
                    .child(
                        div()
                            .flex_none()
                            .w(s(BADGE_SIZE))
                            .text_color(theme.line_number)
                            .child("…"),
                    )
                    .child(div().flex_1().child("Not covered"))
                    .child(div().text_color(theme.line_number).child(files.to_string()))
                    .into_any_element(),
            };
            rows.push(element);
        }
        let settings = cx.global::<Settings>();
        let mut footer = format!(
            "{} · {}",
            settings.walkthrough.model, settings.walkthrough.language
        );
        if let Some(cost) = walkthrough.cost {
            footer += &format!(" · ${cost:.3}");
        }
        div()
            .id("walkthrough-contents")
            .flex_none()
            .w(s(WIDTH))
            .h_full()
            .overflow_y_scroll()
            .p(s(PADDING))
            .border_r_1()
            .border_color(theme.indent_guide)
            .text_size(s(TEXT_SIZE))
            .flex()
            .flex_col()
            .children(rows)
            .child(
                div()
                    .pt(s(PADDING))
                    .text_size(s(SMALL_SIZE))
                    .text_color(theme.line_number)
                    .child(footer),
            )
    }

    /// A row that goes to `page` when clicked.
    fn row(&self, page: usize, shown: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = |v: f32| px(v * self.scale);
        let theme = cx.global::<Settings>().theme;
        div()
            .id(("walkthrough-page", page))
            .h(s(ROW_HEIGHT))
            .px(s(PADDING / 3.))
            .flex()
            .items_center()
            .gap(s(PADDING / 2.))
            .rounded(s(4.))
            .cursor_pointer()
            .when_some(row_bg(&theme, shown, shown, true), |d, bg| d.bg(bg))
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx)))
    }
}
