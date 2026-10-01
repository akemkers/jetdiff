//! Tastelinja med de viktigste tastene, og alle tastene i en popup på `?`.

use gpui::{Div, FontWeight, actions, div, prelude::*, px};

use crate::text::FAMILY;
use crate::theme::UiTheme;

actions!(jetdiff, [ToggleHelp, HideHelp]);

/// Høyden på tastelinja nederst.
pub const BAR_HEIGHT: f32 = 24.;

/// Mål i popupen. Gir to kolonner med seksjoner.
const KEYS_WIDTH: f32 = 110.;
const SECTION_WIDTH: f32 = 290.;
const SECTION_GAP: f32 = 32.;

type Keys = (&'static str, &'static str);

const BAR: [Keys; 5] = [
    ("Tab Shift-Tab", "fil"),
    ("j k", "opp/ned"),
    ("n p", "endring"),
    ("?", "alle taster"),
    ("q", "avslutt"),
];

/// Ctrl-variantene fra vim er ikke med.
const ALL: [(&str, &[Keys]); 4] = [
    (
        "Flytt",
        &[
            ("j k", "opp/ned"),
            ("h l", "sideveis"),
            ("0 $", "linjestart/-slutt"),
            ("d u", "halv side"),
            ("gg G", "topp og bunn"),
            ("n p", "neste/forrige endring"),
            ("H L", "venstre/høyre side"),
        ],
    ),
    (
        "Filer",
        &[
            ("Tab Shift-Tab", "neste/forrige fil"),
            ("J K", "neste/forrige fil"),
            ("C-hjkl", "bytt panel"),
            ("b", "skjul panel"),
        ],
    ),
    ("Folds", &[("o", "åpne fold"), ("f", "toggle folds")]),
    (
        "Annet",
        &[
            ("y", "kopier fil:linje"),
            ("r", "rull tilbake endring"),
            ("Cmd +/-/0", "zoom"),
            ("q", "avslutt"),
        ],
    ),
];

pub struct KeyHelp {
    pub theme: UiTheme,
}

impl KeyHelp {
    pub fn bar(&self) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .min_h(px(BAR_HEIGHT))
            .px(px(10.))
            .py(px(4.))
            .gap_x(px(16.))
            .gap_y(px(2.))
            .items_center()
            .bg(theme.editor_bg)
            .font_family(FAMILY)
            .text_size(px(12.))
            .text_color(theme.line_number)
            .children(BAR.iter().map(|&(keys, what)| {
                div()
                    .flex()
                    .gap(px(6.))
                    .child(div().text_color(theme.editor_fg).child(keys))
                    .child(what)
            }))
    }

    /// Dekker forelderen og legger popupen i midten.
    pub fn popup(&self) -> Div {
        let theme = self.theme;
        let sections = ALL.iter().map(|(title, keys)| {
            let rows = keys.iter().map(|&(keys, what)| {
                div()
                    .flex()
                    .child(
                        div()
                            .w(px(KEYS_WIDTH))
                            .text_color(theme.editor_fg)
                            .child(keys),
                    )
                    .child(what)
            });
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .w(px(SECTION_WIDTH))
                .child(
                    div()
                        .pb(px(2.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.editor_fg)
                        .child(*title),
                )
                .children(rows)
        });
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .px(px(20.))
                    .py(px(16.))
                    .bg(theme.editor_bg)
                    .border_1()
                    .border_color(theme.line_number)
                    .rounded(px(6.))
                    .shadow_lg()
                    .font_family(FAMILY)
                    .text_size(px(13.))
                    .text_color(theme.line_number)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .max_w(px(2. * SECTION_WIDTH + SECTION_GAP))
                            .gap_x(px(SECTION_GAP))
                            .gap_y(px(14.))
                            .children(sections),
                    )
                    .child("? eller Esc lukker"),
            )
    }
}
