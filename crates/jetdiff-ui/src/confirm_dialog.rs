//! Asks before something is done, like a rollback. Whoever shows it handles Enter and Esc.

use gpui::{Div, SharedString, div, prelude::*, px, relative};

use crate::text::FAMILY;
use crate::theme::UiTheme;

/// Long questions wrap. The width is fixed, since GPUI measures text without one as a single line,
/// and the wrapped lines would run out of the box.
const WIDTH: f32 = 420.;

pub fn confirm_dialog(question: impl Into<SharedString>, theme: &UiTheme) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(relative(0.2))
        .child(
            div()
                .w(px(WIDTH))
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(16.))
                .py(px(12.))
                .bg(theme.editor_bg)
                .border_1()
                .border_color(theme.line_number)
                .rounded(px(6.))
                .shadow_lg()
                .font_family(FAMILY)
                .text_size(px(13.))
                .text_color(theme.editor_fg)
                .child(question.into())
                .child(
                    div()
                        .text_color(theme.line_number)
                        .child("[Enter] yes / [Esc] no"),
                ),
        )
}
