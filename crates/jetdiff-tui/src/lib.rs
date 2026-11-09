//! The terminal view, with one row per line.

mod app;
mod backend;
mod clipboard;
mod code_viewer;
mod color;
mod diff_divider_grab;
mod diff_view;
mod geometry;
mod icons;
mod input;
mod side_panel;
mod text;
mod widgets;

use std::io::{self, BufWriter, Stdout};

use jetdiff_app::{Commits, RepoSource};
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::Terminal;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

use crate::app::App;
use crate::backend::TermBackend;

/// Rows per notch on the scroll wheel, in the diff and in the side panel.
const WHEEL_ROWS: usize = 3;

pub struct Settings {
    pub theme: Theme,
    pub unified: bool,
    /// Folds closed from the start. `f` switches for the session.
    pub fold_unchanged: bool,
    pub side_panel_open: bool,
    /// Nerd Font characters in front of the files in the tree.
    pub icons: bool,
    /// Animated jumps, in whole rows.
    pub smooth_scroll: bool,
}

pub fn run_file_pair(
    diff: FileDiff,
    paths: [String; 2],
    title: &str,
    settings: Settings,
) -> io::Result<()> {
    run(App::file(diff, paths, &settings), title)
}

/// `commits` only for a folder, not for a commit or a PR.
pub fn run_repo_source(
    repo_source: RepoSource,
    commits: Option<Commits>,
    title: &str,
    settings: Settings,
) -> io::Result<()> {
    run(App::repo_source(repo_source, commits, &settings), title)
}

/// The whole frame in one write, and the terminal shows it only when it is done. Without this
/// the rows are drawn visibly one after another.
pub type Term = Terminal<TermBackend<BufWriter<Stdout>>>;

fn run(app: App, title: &str) -> io::Result<()> {
    // `init` restores the terminal also on panic
    drop(ratatui::init());
    let backend = TermBackend::new(BufWriter::with_capacity(1 << 20, io::stdout()));
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), SetTitle(title), EnableMouseCapture)?;
    let result = app.run(&mut terminal);
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    ratatui::restore();
    result
}
