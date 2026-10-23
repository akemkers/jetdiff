//! The terminal view.

use jetdiff_tui::Settings;

use crate::Content;
use crate::config::Config;

pub fn run(content: Content, title: &str, config: Config) -> Result<(), String> {
    let settings = Settings {
        theme: config.theme,
        unified: config.unified,
        side_panel_open: config.side_panel.open,
        icons: config.terminal.icons,
    };
    match content {
        Content::File(diff, paths) => jetdiff_tui::run_file(*diff, paths, title, settings),
        Content::Changes(repo, changed_files) => {
            jetdiff_tui::run_changes(repo, changed_files, title, settings)
        }
    }
    .map_err(|e| e.to_string())
}
