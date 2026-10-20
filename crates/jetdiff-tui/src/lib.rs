//! Terminalvisningen, med én rad per linje.

mod app;
mod clipboard;
mod color;
mod diff_view;
mod geometry;
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
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

use crate::app::App;

pub struct Settings {
    pub theme: Theme,
    pub unified: bool,
    pub side_panel_open: bool,
}

pub fn run_file(diff: FileDiff, paths: [String; 2], settings: Settings) -> io::Result<()> {
    run(App::file(diff, paths, &settings))
}

pub fn run_changes(repo: Repo, changes: Vec<FileChange>, settings: Settings) -> io::Result<()> {
    run(App::changes(repo, changes, &settings))
}

/// Hele framen i én skriving, og terminalen viser den først når den er ferdig. Uten dette
/// tegnes radene synlig én etter én.
pub type Term = Terminal<CrosstermBackend<BufWriter<Stdout>>>;

fn run(app: App) -> io::Result<()> {
    // `init` gjenoppretter terminalen også ved panikk
    drop(ratatui::init());
    let backend = CrosstermBackend::new(BufWriter::with_capacity(1 << 20, io::stdout()));
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), EnableMouseCapture)?;
    let result = app.run(&mut terminal);
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    ratatui::restore();
    result
}
