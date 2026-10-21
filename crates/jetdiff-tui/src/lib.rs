//! Terminalvisningen, med én rad per linje.

mod app;
mod backend;
mod clipboard;
mod color;
mod diff_view;
mod geometry;
mod icons;
mod input;
mod message;
mod paint;
mod panel;
mod text;
mod widgets;

use std::io::{self, BufWriter, Stdout};

use jetdiff_changes::FileChange;
use jetdiff_git::Repo;
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::Terminal;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

use crate::app::App;
use crate::backend::TermBackend;

/// Rader per hakk på scrollhjulet, i diffen og i panelet.
const WHEEL_ROWS: usize = 3;

pub struct Settings {
    pub theme: Theme,
    pub unified: bool,
    pub side_panel_open: bool,
    /// Nerd Font-tegn foran filene i treet.
    pub icons: bool,
}

pub fn run_file(
    diff: FileDiff,
    paths: [String; 2],
    title: &str,
    settings: Settings,
) -> io::Result<()> {
    run(App::file(diff, paths, &settings), title)
}

pub fn run_changes(
    repo: Repo,
    changes: Vec<FileChange>,
    title: &str,
    settings: Settings,
) -> io::Result<()> {
    run(App::changes(repo, changes, &settings), title)
}

/// Hele framen i én skriving, og terminalen viser den først når den er ferdig. Uten dette
/// tegnes radene synlig én etter én.
pub type Term = Terminal<TermBackend<BufWriter<Stdout>>>;

fn run(app: App, title: &str) -> io::Result<()> {
    // `init` gjenoppretter terminalen også ved panikk
    drop(ratatui::init());
    let backend = TermBackend::new(BufWriter::with_capacity(1 << 20, io::stdout()));
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), SetTitle(title), EnableMouseCapture)?;
    let result = app.run(&mut terminal);
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    ratatui::restore();
    result
}
