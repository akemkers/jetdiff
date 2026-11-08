//! Asks before something is done, like a rollback. Whoever shows it handles Enter and Esc.

use gpui::{Div, SharedString, div, prelude::*, px, relative};

use crate::help::key_label;
use crate::text::SANS;
use crate::theme::UiPalette;

/// Long questions wrap. The width is fixed, since GPUI measures text without one as a single line,
/// and the wrapped lines would run out of the box.
const WIDTH: f32 = 420.;
/// How far down the window the dialog starts.
const TOP_FRACTION: f32 = 0.2;
const GAP: f32 = 6.;
const HORIZONTAL_PADDING: f32 = 16.;
const VERTICAL_PADDING: f32 = 12.;
const RADIUS: f32 = 6.;
const TEXT_SIZE: f32 = 13.;
const LABEL_GAP: f32 = 4.;

pub fn confirm_dialog(question: impl Into<SharedString>, palette: &UiPalette) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(relative(TOP_FRACTION))
        .child(
            div()
                .w(px(WIDTH))
                .flex()
                .flex_col()
                .gap(px(GAP))
                .px(px(HORIZONTAL_PADDING))
                .py(px(VERTICAL_PADDING))
                .bg(palette.surface)
                .border_1()
                .border_color(palette.border)
                .rounded(px(RADIUS))
                .shadow_lg()
                .font_family(SANS)
                .text_size(px(TEXT_SIZE))
                .text_color(palette.primary)
                .child(question.into())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(LABEL_GAP))
                        .text_color(palette.muted)
                        // The dialog doesn't follow the zoom, like the help popup
                        .child(key_label("Enter", palette, 1.))
                        .child("yes")
                        .child(div().ml(px(LABEL_GAP)).child(key_label("Esc", palette, 1.)))
                        .child("no"),
                ),
        )
}
