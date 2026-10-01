//! Innstillinger for hele appen. Den som åpner vinduet setter dem med `cx.set_global`.

use gpui::Global;

use crate::theme::UiTheme;

pub struct Settings {
    pub theme: UiTheme,
    /// Nye filer vises med alle folds åpne.
    pub folds_open: bool,
}

impl Global for Settings {}
