//! Asks whether the hunk should be rolled back. `FileDiffView` handles Enter and Esc.

use gpui::{Div, div, prelude::*, px, relative};

use crate::text::FAMILY;
use crate::theme::UiTheme;

pub fn rollback_dialog(theme: &UiTheme) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(relative(0.2))
        .child(
            div()
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
                .child("Roll back this hunk?")
                .child(
                    div()
                        .text_color(theme.line_number)
                        .child("[Enter] yes / [Esc] no"),
                ),
        )
}
