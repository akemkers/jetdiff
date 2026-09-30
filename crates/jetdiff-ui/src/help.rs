//! Tastelinja med de viktigste tastene, og alle tastene i en popup på `?`.

use gpui::{Div, FontWeight, actions, div, prelude::*, px};

use crate::theme::UiTheme;

actions!(jetdiff, [ToggleHelp, HideHelp]);

/// Høyden på tastelinja nederst.
pub const BAR_HEIGHT: f32 = 24.;

/// Mål i popupen. Gir to kolonner med seksjoner.
const KEYS_WIDTH: f32 = 110.;
const SECTION_WIDTH: f32 = 290.;
const SECTION_GAP: f32 = 32.;

/// Når en tast vises.
#[derive(Clone, Copy)]
enum Only {
    Always,
    /// Diffen er en del av et filtre.
    Files,
    /// Begge sider vises.
    Split,
}

type Keys = (&'static str, &'static str, Only);

const BAR: [Keys; 5] = [
    ("Tab Shift-Tab", "fil", Only::Files),
    ("j k", "opp/ned", Only::Always),
    ("n p", "endring", Only::Always),
    ("?", "alle taster", Only::Always),
    ("q", "avslutt", Only::Always),
];

/// Ctrl-variantene fra vim er ikke med.
const ALL: [(&str, &[Keys]); 4] = [
    (
        "Flytt",
        &[
            ("j k", "opp/ned", Only::Always),
            ("h l", "sideveis", Only::Always),
            ("0 $", "linjestart/-slutt", Only::Always),
            ("d u", "halv side", Only::Always),
            ("gg G", "topp og bunn", Only::Always),
            ("n p", "neste/forrige endring", Only::Always),
            ("H L", "venstre/høyre side", Only::Split),
        ],
    ),
    (
        "Filer",
        &[
            ("Tab Shift-Tab", "neste/forrige fil", Only::Files),
            ("J K", "neste/forrige fil", Only::Files),
            ("C-hjkl", "bytt panel", Only::Files),
            ("b", "skjul panel", Only::Files),
        ],
    ),
    (
        "Folds",
        &[
            ("o", "åpne fold", Only::Always),
            ("f", "toggle folds", Only::Always),
        ],
    ),
    (
        "Annet",
        &[
            ("y", "kopier fil:linje", Only::Always),
            ("r", "rull tilbake endring", Only::Files),
            ("Cmd +/-/0", "zoom", Only::Always),
            ("q", "avslutt", Only::Always),
        ],
    ),
];

pub struct KeyHelp {
    /// Diffen er en del av et filtre.
    pub files: bool,
    /// Begge sider vises.
    pub split: bool,
    pub theme: UiTheme,
    pub font_family: &'static str,
}

impl KeyHelp {
    fn shows(&self, only: Only) -> bool {
        match only {
            Only::Always => true,
            Only::Files => self.files,
            Only::Split => self.split,
        }
    }

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
            .font_family(self.font_family)
            .text_size(px(12.))
            .text_color(theme.line_number)
            .children(
                BAR.iter()
                    .filter(|k| self.shows(k.2))
                    .map(|&(keys, what, _)| {
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(div().text_color(theme.editor_fg).child(keys))
                            .child(what)
                    }),
            )
    }

    /// Dekker forelderen og legger popupen i midten.
    pub fn popup(&self) -> Div {
        let theme = self.theme;
        let sections = ALL.iter().filter_map(|(title, keys)| {
            let mut rows = keys.iter().filter(|k| self.shows(k.2)).peekable();
            rows.peek()?;
            let rows = rows.map(|&(keys, what, _)| {
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
            Some(
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
                    .children(rows),
            )
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
                    .font_family(self.font_family)
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
