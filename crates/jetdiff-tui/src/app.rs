//! The event loop: keys and mouse, the watcher and reading the repo source in the background.

use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use jetdiff_app::watcher::POLL;
use jetdiff_app::{
    AppCommand, Applied, Commits, Files, Level, ReadJob, RepoRead, RepoSource, Show, Shown,
    Watcher, app_command, empty_diff,
};
use jetdiff_files::ChangedFile;
use jetdiff_git::Repo;
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::Rect;

use crate::clipboard;
use crate::diff_view::{DiffView, Effect, Overlay};
use crate::input::to_key;
use crate::message::Message;
use crate::panel::{Click, ListKey, Panel, Section, list_key};
use crate::widgets;
use crate::{Settings, Term};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Diff,
    Panel(Section),
}

/// The answer from a read in the background.
struct ReadAnswer {
    generation: u64,
    read: Result<RepoRead, String>,
}

struct Repository {
    repo_source: RepoSource,
    panel: Panel,
    selected: Option<usize>,
    panel_open: bool,
    watcher: Option<Watcher>,
    read_sender: Sender<ReadAnswer>,
    read_receiver: Receiver<ReadAnswer>,
    /// Only the answer to the last read is used.
    generation: u64,
}

impl Repository {
    fn read(&mut self, job: ReadJob) {
        self.generation += 1;
        let generation = self.generation;
        let sender = self.read_sender.clone();
        thread::spawn(move || {
            let read = job.run();
            let _ = sender.send(ReadAnswer { generation, read });
        });
    }

    /// The answer to the last read, if it has arrived. Answers to older reads are dropped.
    fn latest_read(&mut self) -> Option<ReadAnswer> {
        let mut latest = None;
        while let Ok(answer) = self.read_receiver.try_recv() {
            if answer.generation == self.generation {
                latest = Some(answer);
            }
        }
        latest
    }
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
            diff: DiffView::new(diff, paths, settings.unified),
            message: Message::default(),
            repository: None,
            focus: Focus::Diff,
            panel_area: Rect::default(),
            quit: false,
        }
    }

    pub fn repo_source(repo: Repo, changed_files: Vec<ChangedFile>, settings: &Settings) -> Self {
        let mut message = Message::default();
        let repo_source = RepoSource::new(repo, changed_files);
        let mut commits = None;
        let mut watcher = None;
        let repo = repo_source.repo();
        if repo.is_local_changes() {
            commits = Some(Commits::new(repo.commits().unwrap_or_else(|e| {
                message.show(e, Level::Warning);
                Vec::new()
            })));
            match Watcher::new(repo.root()) {
                Ok(started) => watcher = Some(started),
                Err(e) => message.show(format!("Can't watch for changes: {e}"), Level::Warning),
            }
        }
        let files = Files::new(repo_source.changed_files().clone());
        let panel = Panel::new(files, commits, settings.icons);
        let selected = repo_source.first();
        let (read_sender, read_receiver) = channel();
        let mut app = Self {
            message,
            repository: Some(Repository {
                repo_source,
                panel,
                selected,
                panel_open: settings.side_panel_open,
                watcher,
                read_sender,
                read_receiver,
                generation: 0,
            }),
            ..Self::file(empty_diff(), Default::default(), settings)
        };
        app.select(selected);
        app
    }

    pub fn run(mut self, terminal: &mut Term) -> io::Result<()> {
        while !self.quit {
            queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
            terminal.draw(|frame| self.draw(frame))?;
            execute!(terminal.backend_mut(), EndSynchronizedUpdate)?;
            let mut timeout = POLL;
            while event::poll(timeout)? {
                self.event(event::read()?)?;
                timeout = Duration::ZERO;
            }
            self.background();
        }
        Ok(())
    }

    fn background(&mut self) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        if let Some(paths) = repo.watcher.as_ref().and_then(|watcher| watcher.take()) {
            let job = repo.repo_source.files_changed(paths);
            repo.read(job);
        }
        if let Some(answer) = repo.latest_read() {
            let applied = repo.repo_source.apply(answer.read);
            self.show_applied(applied);
        }
    }

    fn show_applied(&mut self, applied: Applied) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        match applied {
            Applied::Failed {
                selected_commit,
                error,
            } => {
                if let Some(commits) = &mut repo.panel.commits {
                    commits.set_selected(selected_commit);
                }
                self.message.show(error, Level::Warning);
            }
            Applied::Done {
                commits,
                changed_files,
                file,
                notice,
            } => {
                if let Some(list) = &mut repo.panel.commits {
                    list.set_commits(commits);
                }
                if let Some(changed_files) = changed_files {
                    repo.panel.files.set_changed_files(changed_files);
                }
                self.select(file);
                if let Some(notice) = notice {
                    self.message.show(notice.text, notice.level);
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
        let show = repo.repo_source.load(file);
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
                self.message.show(format!("Copied {location}"), Level::Info);
            }
            Some(Effect::Rollback(request)) => {
                let Some(repo) = &mut self.repository else {
                    return Ok(());
                };
                match repo.repo_source.rollback(&request) {
                    None => {}
                    Some(Ok(show)) => {
                        let job = repo.repo_source.rolled_back();
                        repo.read(job);
                        self.show(show);
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

    /// As in GPUI: the keys in `jetdiff_app::keys` first, then whatever has focus.
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

    /// `false` when the key should pass through.
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
                    && let Some(next) = repo.panel.files.next_file(file, forward)
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
                    && let Some(section) = [Section::Files, Section::Commits]
                        .into_iter()
                        .find(|&s| self.section_open(s))
                {
                    self.focus = Focus::Panel(section);
                }
            }
            AppCommand::FocusRight => self.focus = Focus::Diff,
            AppCommand::FocusDown | AppCommand::FocusUp => {
                let (from, to) = match cmd {
                    AppCommand::FocusDown => (Section::Files, Section::Commits),
                    _ => (Section::Commits, Section::Files),
                };
                if self.focus == Focus::Panel(from) && self.section_open(to) {
                    self.focus = Focus::Panel(to);
                }
            }
        }
        Ok(true)
    }

    fn section_open(&self, section: Section) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|r| r.panel_open && r.panel.is_open(section))
    }

    fn list_key(&mut self, section: Section, key: jetdiff_viewmodel::Key) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        match list_key(key) {
            Some(ListKey::Down) => repo.panel.move_cursor(section, true),
            Some(ListKey::Up) => repo.panel.move_cursor(section, false),
            Some(ListKey::Select) => match section {
                Section::Files => {
                    if let Some(file) = repo.panel.files.select_at_cursor() {
                        self.select(Some(file));
                    }
                }
                Section::Commits => {
                    let hash = repo
                        .panel
                        .commits
                        .as_mut()
                        .and_then(|commits| commits.select_at_cursor());
                    self.select_commit(hash);
                }
            },
            None => {}
        }
    }

    fn select_commit(&mut self, hash: Option<Option<String>>) {
        if let (Some(hash), Some(repo)) = (hash, &mut self.repository) {
            let job = repo.repo_source.select_commit(hash);
            repo.read(job);
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
        match repo.panel.click(y) {
            Some(Click::Heading(section)) => {
                repo.panel.toggle(section);
                if self.focus == Focus::Panel(section) && !repo.panel.is_open(section) {
                    self.focus = Focus::Diff;
                }
            }
            Some(Click::File(row)) => {
                self.focus = Focus::Panel(Section::Files);
                if let Some(file) = repo.panel.files.select(row) {
                    self.select(Some(file));
                }
            }
            Some(Click::Commit(row)) => {
                self.focus = Focus::Panel(Section::Commits);
                let hash = repo
                    .panel
                    .commits
                    .as_mut()
                    .and_then(|commits| commits.select(row));
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
            self.diff.wheel(down);
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
            let width = repo.panel.width(area.width);
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
        if let Some(file) = self
            .repository
            .as_ref()
            .and_then(|repository| repository.repo_source.shown())
        {
            widgets::header(&file.path, Rect { height: 1, ..main }, &theme, buf);
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
