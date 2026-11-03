//! The contents on the left: the overview, each part with its steps, and the files no step covers. A
//! step's box has its number, and a check once it has been read. At the bottom, what the marks mean,
//! and how much is read. A click on a row goes to its page.

use gpui::{Context, Div, FontWeight, SharedString, Stateful, div, prelude::*, px, relative};
use jetdiff_app::walkthrough::{ContentsRow, Reading, Weight};

use super::palette::{
    BORDER, KEY_DOT, MUTED, PRIMARY, READ, SELECTED, STEP_NUMBER, STRONG, SURFACE, color,
};
use super::prose::SANS;
use super::{WalkthroughView, plural};
use crate::text::FAMILY;

const WIDTH: f32 = 300.;
const PADDING: f32 = 14.;
const TEXT_SIZE: f32 = 14.;
const LINE_HEIGHT: f32 = 20.;
const LABEL_SIZE: f32 = 11.;
const SMALL_SIZE: f32 = 12.;
/// Rows are easy to hit.
const MIN_ROW_HEIGHT: f32 = 40.;
const BADGE_SIZE: f32 = 20.;
const KEY_DOT_SIZE: f32 = 8.;
const PROGRESS_HEIGHT: f32 = 4.;

impl WalkthroughView {
    pub(super) fn contents(&self, reading: &Reading, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let mut rows = Vec::new();
        for row in reading.contents() {
            let element = match row {
                ContentsRow::Overview { selected } => self
                    .row(0, selected, cx)
                    .child(self.badge("◆".into(), false))
                    .child(title("Overview".into(), selected, false))
                    .into_any_element(),
                ContentsRow::Part(part) => div()
                    .pt(s(PADDING * 1.25))
                    .pb(s(PADDING / 3.))
                    .px(s(PADDING / 2.))
                    .text_size(s(LABEL_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(MUTED))
                    .child(part.title.to_uppercase())
                    .into_any_element(),
                ContentsRow::Step {
                    page,
                    number,
                    step,
                    read,
                    selected,
                } => {
                    let checked = read && !selected;
                    let badge = if checked {
                        "✓".into()
                    } else {
                        number.to_string()
                    };
                    let skim = step.weight == Weight::Skim;
                    self.row(page, selected, cx)
                        .child(self.badge(badge, checked))
                        .child(title(step.title.clone(), selected, skim))
                        .when(step.is_key(), |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .mt(s((LINE_HEIGHT - KEY_DOT_SIZE) / 2.))
                                    .size(s(KEY_DOT_SIZE))
                                    .rounded_full()
                                    .bg(color(KEY_DOT)),
                            )
                        })
                        .into_any_element()
                }
                ContentsRow::NotCovered {
                    page,
                    files,
                    selected,
                } => div()
                    .mt(s(PADDING))
                    .pt(s(PADDING / 2.))
                    .border_t_1()
                    .border_color(color(BORDER))
                    .child(
                        self.row(page, selected, cx)
                            .child(self.badge("…".into(), false))
                            .child(title("Not covered".into(), selected, false))
                            .child(pill(plural(files, "file"), self.scale)),
                    )
                    .into_any_element(),
            };
            rows.push(element);
        }
        div()
            .flex_none()
            .w(s(WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(color(SURFACE))
            .border_r_1()
            .border_color(color(BORDER))
            .font_family(SANS)
            .text_size(s(TEXT_SIZE))
            .line_height(s(LINE_HEIGHT))
            .text_color(color(PRIMARY))
            .child(
                div()
                    .id("walkthrough-contents")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(s(PADDING / 2.))
                    .flex()
                    .flex_col()
                    .gap(s(2.))
                    .children(rows),
            )
            .child(self.key(reading))
    }

    /// A row that goes to `page` when clicked. The selected one stands out.
    fn row(&self, page: usize, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = |v: f32| px(v * self.scale);
        div()
            .id(("walkthrough-page", page))
            .min_h(s(MIN_ROW_HEIGHT))
            .px(s(PADDING / 2.))
            .py(s((MIN_ROW_HEIGHT - LINE_HEIGHT) / 2.))
            .flex()
            .items_start()
            .gap(s(PADDING * 0.75))
            .rounded(s(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(color(SELECTED)))
            .when(!selected, |d| d.hover(|d| d.bg(color(BORDER))))
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx)))
    }

    /// The step's number, or a check once it is read.
    fn badge(&self, text: String, read: bool) -> Div {
        let s = |v: f32| px(v * self.scale);
        div()
            .flex_none()
            .size(s(BADGE_SIZE))
            .rounded(s(5.))
            .flex()
            .items_center()
            .justify_center()
            .bg(color(BORDER))
            .font_family(FAMILY)
            .text_size(s(LABEL_SIZE))
            .text_color(if read {
                color(READ)
            } else {
                color(STEP_NUMBER)
            })
            .child(text)
    }

    /// What the marks mean, and how much is read.
    fn key(&self, reading: &Reading) -> Div {
        let s = |v: f32| px(v * self.scale);
        let (read, steps) = (reading.read_count(), reading.step_count());
        let read_share = read as f32 / steps.max(1) as f32;
        let mark = |mark: Div, text: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(s(6.))
                .child(mark)
                .child(text)
        };
        div()
            .flex_none()
            .p(s(PADDING))
            .flex()
            .flex_col()
            .gap(s(8.))
            .border_t_1()
            .border_color(color(BORDER))
            .text_size(s(SMALL_SIZE))
            .text_color(color(MUTED))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(s(PADDING))
                    .gap_y(s(4.))
                    .child(mark(div().text_color(color(READ)).child("✓"), "Read"))
                    .child(mark(
                        div()
                            .size(s(KEY_DOT_SIZE))
                            .rounded_full()
                            .bg(color(KEY_DOT)),
                        "Key step",
                    ))
                    .child(mark(div(), "Dimmed: safe to skim")),
            )
            .child(format!("{read} of {steps} read"))
            .child(
                div()
                    .h(s(PROGRESS_HEIGHT))
                    .rounded_full()
                    .bg(color(BORDER))
                    .child(
                        div()
                            .h_full()
                            .w(relative(read_share))
                            .rounded_full()
                            .bg(color(READ)),
                    ),
            )
    }
}

/// Wraps instead of being cut, so the whole title shows.
fn title(text: String, selected: bool, skim: bool) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .when(skim && !selected, |d| d.text_color(color(MUTED)))
        .when(selected, |d| {
            d.text_color(color(STRONG))
                .font_weight(FontWeight::SEMIBOLD)
        })
        .child(SharedString::from(text))
}

fn pill(text: String, scale: f32) -> Div {
    div()
        .flex_none()
        .px(px(8. * scale))
        .rounded_full()
        .bg(color(BORDER))
        .text_size(px(SMALL_SIZE * scale))
        .text_color(color(MUTED))
        .child(text)
}
