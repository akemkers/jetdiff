//! Terminalvisningen.

use jetdiff_tui::Settings;

use crate::Content;
use crate::config::Config;

pub fn run(content: Content, title: &str, config: Config) -> Result<(), String> {
    let settings = Settings {
        theme: config.theme,
        unified: config.unified,
        side_panel_open: config.side_panel.open,
    };
    match content {
        Content::File(diff, paths) => jetdiff_tui::run_file(*diff, paths, title, settings),
        Content::Changes(repo, changes) => jetdiff_tui::run_changes(repo, changes, title, settings),
    }
    .map_err(|e| e.to_string())
}
