//! Settings for the whole app. Whoever opens the window sets them with `cx.set_global`.

use gpui::Global;

use jetdiff_app::Config;
use jetdiff_app::walkthrough::Writer;
use jetdiff_viewmodel::Theme;

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

impl Settings {
    /// `version` is shown at the right of the keymap bar.
    pub fn new(config: &Config, version: &'static str) -> Self {
        Self {
            theme: UiTheme::new(&Theme::built_in()),
            font_size: config.font_size,
            fold_unchanged: config.fold_unchanged,
            unified: config.unified,
            side_panel_open: config.side_panel.open,
            walkthrough: Writer {
                command: config.walkthrough.command.clone(),
                model: config.walkthrough.model.clone(),
                language: config.walkthrough.language.clone(),
            },
            version,
        }
    }
}
