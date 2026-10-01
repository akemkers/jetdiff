//! Innstillinger for hele appen. Den som åpner vinduet setter dem med `cx.set_global`.

use gpui::Global;

use crate::theme::UiTheme;

pub struct Settings {
    pub theme: UiTheme,
    /// Der zoom starter, og der `ResetZoom` går tilbake.
    pub font_size: f32,
    /// Nye filer vises med uendrede deler klappet sammen.
    pub collapse_unchanged: bool,
    pub side_panel_open: bool,
}

impl Global for Settings {}
