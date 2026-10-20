//! Hendelsesløkka: taster og mus, watcheren og reload i bakgrunnen.

use std::io;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Instant;

use jetdiff_app::watcher::POLL;
use jetdiff_app::{
    AppCommand, Changes, Commits, FileTree, Level, Loaded, Reloaded, Show, Shown, Watcher,
    app_command, empty_diff,
};
use jetdiff_changes::FileChange;
use jetdiff_git::Repo;
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Rect;

use crate::Settings;
use crate::clipboard;
use crate::diff_view::{DiffView, Effect, Overlay};
use crate::input::to_key;
use crate::message::Message;
use crate::panel::{Click, ListKey, Panel, Section, list_key};
use crate::widgets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Diff,
    Panel(Section),
}

type ReloadResult = (u64, Result<Loaded, String>, Vec<String>);

struct Repository {
    changes: Changes,
    panel: Panel,
    selected: Option<usize>,
    panel_open: bool,
    watcher: Option<Watcher>,
    reloads: (
        std::sync::mpsc::Sender<ReloadResult>,
        Receiver<ReloadResult>,
    ),
    /// Bare svaret på siste reload brukes.
    generation: u64,
}

pub struct App {
    theme: Theme,
    diff: DiffView,
    message: Message,
    repository: Option<Repository>,
    focus: Focus,
    panel_area: Rect,
    quit: bool,
}

impl App {
    pub fn file(diff: FileDiff, paths: [String; 2], settings: &Settings) -> Self {
        Self {
            theme: settings.theme,
            diff: DiffView::new(diff, paths, settings.unified, settings.collapse_unchanged),
            message: Message::default(),
            repository: None,
            focus: Focus::Diff,
            panel_area: Rect::default(),
            quit: false,
        }
    }

    pub fn changes(repo: Repo, changes: Vec<FileChange>, settings: &Settings) -> Self {
        let mut message = Message::default();
        let changes = Changes::new(repo, changes);
        let mut commits = None;
        let mut watcher = None;
        let repo = changes.repo();
        if repo.is_work_tree() {
            commits = Some(Commits::new(repo.commits().unwrap_or_else(|e| {
                message.show(e, Level::Warning);
                Vec::new()
            })));
            match Watcher::new(repo.root()) {
                Ok(w) => watcher = Some(w),
                Err(e) => message.show(format!("Følger ikke endringer: {e}"), Level::Warning),
            }
        }
        let panel = Panel::new(FileTree::new(changes.changes().clone()), commits);
        let selected = changes.first();
        let mut app = Self {
            repository: Some(Repository {
                changes,
                panel,
                selected,
                panel_open: settings.side_panel_open,
                watcher,
                reloads: channel(),
                generation: 0,
            }),
            ..Self::file(empty_diff(), Default::default(), settings)
        };
        app.message = message;
        app.select(selected);
        app
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.quit {
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(POLL)? {
                loop {
                    self.event(event::read()?)?;
                    if !event::poll(std::time::Duration::ZERO)? {
                        break;
                    }
                }
            }
            self.background();
        }
        Ok(())
    }

    fn background(&mut self) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        if let Some(paths) = repo.watcher.as_ref().and_then(|w| w.take()) {
            self.reload(paths);
        }
        let Some(repo) = &mut self.repository else {
            return;
        };
        let mut latest = None;
        while let Ok(result) = repo.reloads.1.try_recv() {
            if result.0 == repo.generation {
                latest = Some(result);
            }
        }
        if let Some((_, loaded, paths)) = latest {
            let reloaded = repo.changes.reloaded(loaded, &paths);
            self.reloaded(reloaded);
        }
    }

    fn reload(&mut self, paths: Vec<String>) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        repo.generation += 1;
        let (generation, job, tx) = (
            repo.generation,
            repo.changes.reload(),
            repo.reloads.0.clone(),
        );
        thread::spawn(move || {
            let _ = tx.send((generation, job.run(), paths));
        });
    }

    fn reloaded(&mut self, reloaded: Reloaded) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        match reloaded {
            Reloaded::Failed { shown, error } => {
                if let Some(commits) = &mut repo.panel.commits {
                    commits.set_selected(shown);
                }
                self.message.show(error, Level::Warning);
            }
            Reloaded::Done {
                commits,
                changes,
                file,
                notice,
            } => {
                if let Some(list) = &mut repo.panel.commits {
                    list.set_commits(commits);
                }
                if let Some(changes) = changes {
                    repo.panel.tree.set_changes(changes);
                }
                self.select(file);
                if let Some(n) = notice {
                    self.message.show(n.text, n.level);
                }
            }
        }
    }

    fn select(&mut self, file: Option<usize>) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        repo.selected = file;
        if let Some(file) = file {
            repo.panel.reveal_file(file);
        }
        let show = repo.changes.load(file);
        self.show(show);
    }

    fn show(&mut self, show: Show) {
        match show {
            Show::Unchanged => {}
            Show::Empty(notice) => {
                self.message.pin(Some(notice));
                self.diff.set_diff(empty_diff(), Default::default());
            }
            Show::Diff(shown) => {
                let Shown {
                    diff,
                    notice,
                    paths,
                    can_rollback,
                    same_file,
                } = *shown;
                self.message.pin(notice);
                self.diff.set_can_rollback(can_rollback);
                if same_file {
                    self.diff.update_diff(diff, paths);
                } else {
                    self.diff.set_diff(diff, paths);
                }
            }
        }
    }

    fn effect(&mut self, effect: Option<Effect>) -> io::Result<()> {
        match effect {
            None => {}
            Some(Effect::Message(text)) => self.message.show(text, Level::Info),
            Some(Effect::Copy(location)) => {
                clipboard::copy(&location, &mut io::stdout())?;
                self.message
                    .show(format!("Kopierte {location}"), Level::Info);
            }
            Some(Effect::Rollback(request)) => {
                let Some(repo) = &mut self.repository else {
                    return Ok(());
                };
                match repo.changes.rollback(&request) {
                    None => {}
                    Some(Ok(show)) => {
                        self.show(show);
                        self.reload(Vec::new());
                    }
                    Some(Err(text)) => self.message.show(text, Level::Warning),
                }
            }
        }
        Ok(())
    }

    fn event(&mut self, event: Event) -> io::Result<()> {
        match event {
            Event::Key(k) if k.kind != KeyEventKind::Release => self.key(to_key(k)),
            Event::Mouse(m) => {
                match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => self.click(m.column, m.row),
                    MouseEventKind::ScrollDown => self.wheel(m.column, m.row, true),
                    MouseEventKind::ScrollUp => self.wheel(m.column, m.row, false),
                    _ => {}
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Som i GPUI: tastene i `jetdiff_app::keys` først, så det som har fokus.
    fn key(&mut self, key: jetdiff_viewmodel::Key) -> io::Result<()> {
        if let Some(cmd) = app_command(key)
            && self.app_command(cmd)?
        {
            return Ok(());
        }
        match self.focus {
            Focus::Diff => {
                let effect = self.diff.key(key);
                self.effect(effect)
            }
            Focus::Panel(section) => {
                self.list_key(section, key);
                Ok(())
            }
        }
    }

    /// `false` når tasten skal gå videre.
    fn app_command(&mut self, cmd: AppCommand) -> io::Result<bool> {
        match cmd {
            AppCommand::Quit => self.quit = true,
            AppCommand::ToggleHelp => self.diff.toggle_help(),
            AppCommand::HideHelp => return Ok(self.diff.hide_help()),
            AppCommand::ToggleView => {
                let effect = self.diff.toggle_view();
                self.effect(effect)?;
            }
            AppCommand::ZoomIn | AppCommand::ZoomOut | AppCommand::ResetZoom => {}
            AppCommand::NextFile | AppCommand::PrevFile => {
                let forward = cmd == AppCommand::NextFile;
                if let Some(repo) = &self.repository
                    && let Some(file) = repo.selected
                    && let Some(next) = repo.panel.tree.next_file(file, forward)
                {
                    self.select(Some(next));
                }
            }
            AppCommand::ToggleSidePanel => {
                if let Some(repo) = &mut self.repository {
                    repo.panel_open = !repo.panel_open;
                    if !repo.panel_open {
                        self.focus = Focus::Diff;
                    }
                }
            }
            AppCommand::FocusLeft => {
                if self.focus == Focus::Diff
                    && let Some(section) = self.open_sections().first()
                {
                    self.focus = Focus::Panel(*section);
                }
            }
            AppCommand::FocusRight => self.focus = Focus::Diff,
            AppCommand::FocusDown | AppCommand::FocusUp => {
                let (from, to) = match cmd {
                    AppCommand::FocusDown => (Section::Files, Section::Commits),
                    _ => (Section::Commits, Section::Files),
                };
                let open = self.open_sections();
                if self.focus == Focus::Panel(from) && open.contains(&from) && open.contains(&to) {
                    self.focus = Focus::Panel(to);
                }
            }
        }
        Ok(true)
    }

    fn open_sections(&self) -> Vec<Section> {
        let Some(repo) = self.repository.as_ref().filter(|r| r.panel_open) else {
            return Vec::new();
        };
        [Section::Files, Section::Commits]
            .into_iter()
            .filter(|&s| repo.panel.is_open(s))
            .collect()
    }

    fn list_key(&mut self, section: Section, key: jetdiff_viewmodel::Key) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        match list_key(key) {
            Some(ListKey::Down) => repo.panel.move_cursor(section, true),
            Some(ListKey::Up) => repo.panel.move_cursor(section, false),
            Some(ListKey::Open) => match section {
                Section::Files => {
                    if let Some(file) = repo.panel.tree.open_cursor() {
                        self.select(Some(file));
                    }
                }
                Section::Commits => {
                    let hash = repo.panel.commits.as_mut().and_then(|c| c.open_cursor());
                    self.select_commit(hash);
                }
            },
            None => {}
        }
    }

    fn select_commit(&mut self, hash: Option<Option<String>>) {
        if let (Some(hash), Some(repo)) = (hash, &mut self.repository) {
            repo.changes.select_commit(hash);
            self.reload(Vec::new());
        }
    }

    fn panel_contains(&self, x: u16, y: u16) -> bool {
        self.repository.as_ref().is_some_and(|r| r.panel_open)
            && self.panel_area.contains((x, y).into())
    }

    fn click(&mut self, x: u16, y: u16) {
        if !self.panel_contains(x, y) {
            self.focus = Focus::Diff;
            self.diff.click(x, y);
            return;
        }
        let Some(repo) = &mut self.repository else {
            return;
        };
        match repo.panel.click(x, y) {
            Some(Click::Heading(section)) => {
                repo.panel.toggle(section);
                // Fokus i en lukket seksjon går til diffen
                if self.focus == Focus::Panel(section) && !repo.panel.is_open(section) {
                    self.focus = Focus::Diff;
                }
            }
            Some(Click::File(row)) => {
                self.focus = Focus::Panel(Section::Files);
                if let Some(file) = repo.panel.tree.open(row) {
                    self.select(Some(file));
                }
            }
            Some(Click::Commit(row)) => {
                self.focus = Focus::Panel(Section::Commits);
                let hash = repo.panel.commits.as_mut().and_then(|c| c.select(row));
                self.select_commit(hash);
            }
            None => {}
        }
    }

    fn wheel(&mut self, x: u16, y: u16, down: bool) {
        if self.panel_contains(x, y) {
            if let Some(repo) = &mut self.repository {
                repo.panel.wheel(y, down);
            }
        } else {
            self.diff.wheel(x, down);
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let theme = self.theme;
        let area = frame.area();
        let buf = frame.buffer_mut();
        let mut main = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let footer = Rect {
            y: main.bottom(),
            height: area.height.min(1),
            ..area
        };
        let focus = self.focus;
        if let Some(repo) = self.repository.as_mut().filter(|r| r.panel_open) {
            let width = repo.panel.width(area.width * 2 / 5);
            self.panel_area = Rect {
                width: width.min(area.width),
                ..main
            };
            let section = match focus {
                Focus::Panel(s) => Some(s),
                Focus::Diff => None,
            };
            repo.panel
                .render(self.panel_area, section, repo.selected, &theme, buf);
            main.x += self.panel_area.width;
            main.width -= self.panel_area.width;
        }
        if let Some(change) = self.repository.as_ref().and_then(|r| r.changes.shown()) {
            widgets::header(&change.path, Rect { height: 1, ..main }, &theme, buf);
            main.y += 1;
            main.height = main.height.saturating_sub(1);
        }
        self.diff.layout(main);
        self.diff.render(focus == Focus::Diff, &theme, buf);
        widgets::key_bar(footer, &theme, buf);
        if let Some(notice) = self
            .message
            .current(Instant::now())
            .filter(|_| main.height > 0)
        {
            let row = Rect {
                y: main.bottom() - 1,
                height: 1,
                ..main
            };
            widgets::message(notice, row, &theme, buf);
        }
        match &self.diff.overlay {
            Some(Overlay::Help) => widgets::help(area, &theme, buf),
            Some(Overlay::Confirm(_)) => widgets::confirm_rollback(main, &theme, buf),
            None => {}
        }
    }
}
