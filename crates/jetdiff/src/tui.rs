//! The terminal view.

use jetdiff_tui::Settings;
use jetdiff_viewmodel::Theme;

use crate::Content;
use crate::config::Config;

pub fn run(content: Content, title: &str, config: Config) -> Result<(), String> {
    let settings = Settings {
        theme: Theme::built_in(),
        unified: config.unified,
        fold_unchanged: config.fold_unchanged,
        side_panel_open: config.side_panel.open,
        icons: config.terminal.icons,
        smooth_scroll: config.terminal.smooth_scroll,
    };
    match content {
        Content::FilePair(file_pair, diff) => {
            jetdiff_tui::run_file_pair(file_pair, *diff, title, settings)
        }
        Content::Repo(repo_source, commits) => {
            jetdiff_tui::run_repo_source(*repo_source, commits, title, settings)
        }
    }
    .map_err(|e| e.to_string())
}
