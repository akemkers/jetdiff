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

use std::io;

use jetdiff_changes::FileChange;
use jetdiff_git::Repo;
use jetdiff_viewmodel::{FileDiff, Theme};
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

fn run(app: App) -> io::Result<()> {
    // `init` gjenoppretter terminalen også ved panikk
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = app.run(&mut terminal);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
