//! The contents in the side panel: the overview, each part with its steps, and the files no step covers. A
//! step's box has its number, and a check once it has been read. At the bottom, what the marks mean,
//! and how much is read. A click on a row goes to its page.

use gpui::{Context, Div, FontWeight, SharedString, Stateful, div, prelude::*, px, relative};
use jetdiff_app::walkthrough::{ContentsRow, Reading, Weight};

use super::{State, WalkthroughView, plural};
use crate::settings::Settings;
use crate::text::{FAMILY, SANS};
use crate::theme::UiPalette;

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
    /// `None` until the walkthrough is written. The side panel gives the width and the background.
    pub(crate) fn contents(&self, cx: &mut Context<Self>) -> Option<Div> {
        let State::Ready(reading) = &self.state else {
            return None;
        };
        let s = |v: f32| px(v * self.scale);
        let palette = cx.global::<Settings>().theme.palette;
        let mut rows = Vec::new();
        for row in reading.contents() {
            let element = match row {
                ContentsRow::Overview { selected } => self
                    .row(0, selected, cx)
                    .child(self.badge("◆".into(), false, &palette))
                    .child(title("Overview".into(), selected, false, &palette))
                    .into_any_element(),
                ContentsRow::Part(part) => div()
                    .pt(s(PADDING * 1.25))
                    .pb(s(PADDING / 3.))
                    .px(s(PADDING / 2.))
                    .text_size(s(LABEL_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.muted)
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
                        .child(self.badge(badge, checked, &palette))
                        .child(title(step.title.clone(), selected, skim, &palette))
                        .when(step.is_key(), |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .mt(s((LINE_HEIGHT - KEY_DOT_SIZE) / 2.))
                                    .size(s(KEY_DOT_SIZE))
                                    .rounded_full()
                                    .bg(palette.key_dot),
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
                    .border_color(palette.border)
                    .child(
                        self.row(page, selected, cx)
                            .child(self.badge("…".into(), false, &palette))
                            .child(title("Not covered".into(), selected, false, &palette))
                            .child(pill(plural(files, "file"), &palette, self.scale)),
                    )
                    .into_any_element(),
            };
            rows.push(element);
        }
        let contents = div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(SANS)
            .text_size(s(TEXT_SIZE))
            .line_height(s(LINE_HEIGHT))
            .text_color(palette.primary)
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
            .child(self.key(reading, &palette));
        Some(contents)
    }

    /// A row that goes to `page` when clicked. The selected one stands out.
    fn row(&self, page: usize, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = |v: f32| px(v * self.scale);
        let palette = cx.global::<Settings>().theme.palette;
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
            .when(selected, |d| d.bg(palette.selected))
            .when(!selected, |d| d.hover(|d| d.bg(palette.border)))
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx)))
    }

    /// The step's number, or a check once it is read.
    fn badge(&self, text: String, read: bool, palette: &UiPalette) -> Div {
        let s = |v: f32| px(v * self.scale);
        div()
            .flex_none()
            .size(s(BADGE_SIZE))
            .rounded(s(5.))
            .flex()
            .items_center()
            .justify_center()
            .bg(palette.border)
            .font_family(FAMILY)
            .text_size(s(LABEL_SIZE))
            .text_color(if read {
                palette.read
            } else {
                palette.step_number
            })
            .child(text)
    }

    /// What the marks mean, and how much is read.
    fn key(&self, reading: &Reading, palette: &UiPalette) -> Div {
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
            .border_color(palette.border)
            .text_size(s(SMALL_SIZE))
            .text_color(palette.muted)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(s(PADDING))
                    .gap_y(s(4.))
                    .child(mark(div().text_color(palette.read).child("✓"), "Read"))
                    .child(mark(
                        div()
                            .size(s(KEY_DOT_SIZE))
                            .rounded_full()
                            .bg(palette.key_dot),
                        "Key step",
                    ))
                    .child(mark(div(), "Dimmed: safe to skim")),
            )
            .child(format!("{read} of {steps} read"))
            .child(
                div()
                    .h(s(PROGRESS_HEIGHT))
                    .rounded_full()
                    .bg(palette.border)
                    .child(
                        div()
                            .h_full()
                            .w(relative(read_share))
                            .rounded_full()
                            .bg(palette.read),
                    ),
            )
    }
}

/// Wraps instead of being cut, so the whole title shows.
fn title(text: String, selected: bool, skim: bool, palette: &UiPalette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .when(skim && !selected, |d| d.text_color(palette.muted))
        .when(selected, |d| {
            d.text_color(palette.strong)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .child(SharedString::from(text))
}

fn pill(text: String, palette: &UiPalette, scale: f32) -> Div {
    div()
        .flex_none()
        .px(px(8. * scale))
        .rounded_full()
        .bg(palette.border)
        .text_size(px(SMALL_SIZE * scale))
        .text_color(palette.muted)
        .child(text)
}
