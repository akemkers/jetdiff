//! Settings for the whole app. Whoever opens the window sets them with `cx.set_global`.

use gpui::Global;

use jetdiff_app::walkthrough::Writer;

use crate::theme::UiTheme;

pub struct Settings {
    pub theme: UiTheme,
    /// Where zoom starts, and where `ResetZoom` goes back to.
    pub font_size: f32,
    /// Where folding starts. `f` switches for the session.
    pub fold_unchanged: bool,
    /// The diff mode at startup. `v` switches.
    pub unified: bool,
    pub side_panel_open: bool,
    pub walkthrough: Writer,
    /// Shown at the right of the keymap bar.
    pub version: &'static str,
}

impl Global for Settings {}
