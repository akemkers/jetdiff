//! Linja over diffen med stien til fila som vises.

use gpui::{Div, FontWeight, div, prelude::*, px};

use crate::list::TEXT_SIZE;
use crate::text::FAMILY;
use crate::theme::UiTheme;

/// Mappa kuttes når det blir trangt, filnavnet står alltid.
pub fn file_header(path: &str, scale: f32, theme: &UiTheme) -> Div {
    let (dir, name) = match path.rsplit_once('/') {
        Some((dir, name)) => (Some(format!("{dir}/")), name),
        None => (None, path),
    };
    div()
        .flex_none()
        .flex()
        .px(px(10. * scale))
        .py(px(6. * scale))
        .bg(theme.editor_bg)
        .border_b_1()
        .border_color(theme.indent_guide)
        .font_family(FAMILY)
        .text_size(px(TEXT_SIZE * scale))
        .children(dir.map(|d| {
            div()
                .min_w_0()
                .truncate()
                .text_color(theme.line_number)
                .child(d)
        }))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.editor_fg)
                .child(name.to_owned()),
        )
}
