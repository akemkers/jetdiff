//! The event loop: keys and mouse, the watcher and reading the repo source in the background.

use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use jetdiff_app::watcher::POLL;
use jetdiff_app::{
    AppCommand, Applied, Commits, Files, Notification, NotificationSlot, ReadJob, RepoRead,
    RepoSource, Selected, Show, Shown, Watcher, app_command, empty_diff,
};
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::Rect;

use crate::clipboard;
use crate::diff_view::{DiffView, Effect, Overlay};
use crate::input::to_key;
use crate::side_panel::{Click, ListKey, Section, SidePanel, list_key};
use crate::widgets;
use crate::{Settings, Term};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Diff,
    SidePanel(Section),
}

/// The answer from a read in the background.
struct ReadAnswer {
    generation: u64,
    read: Result<RepoRead, String>,
}

struct Repository {
    repo_source: RepoSource,
    side_panel: SidePanel,
    selected: Option<usize>,
    side_panel_open: bool,
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

    /// The answer to the last read, if it has arrived. Answers to earlier reads are dropped.
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
    notification_slot: NotificationSlot,
    repository: Option<Repository>,
    focus: Focus,
    side_panel_area: Rect,
    quit: bool,
}

impl App {
    pub fn file(diff: FileDiff, paths: [String; 2], settings: &Settings) -> Self {
        Self {
            theme: settings.theme,
            diff: DiffView::new(diff, paths, settings.unified),
            notification_slot: NotificationSlot::default(),
            repository: None,
            focus: Focus::Diff,
            side_panel_area: Rect::default(),
            quit: false,
        }
    }

    pub fn repo_source(
        repo_source: RepoSource,
        commits: Option<Commits>,
        settings: &Settings,
    ) -> Self {
        let mut app = Self::file(empty_diff(), Default::default(), settings);
        let mut watcher = None;
        if commits.is_some() {
            match Watcher::new(repo_source.repo()) {
                Ok(started) => watcher = Some(started),
                Err(e) => app.show_brief(Notification::warning(format!(
                    "Can't watch for file changes: {e}"
                ))),
            }
        }
        let files = Files::new(repo_source.changed_files().clone());
        let side_panel = SidePanel::new(files, commits, settings.icons);
        let selected = repo_source.first();
        let (read_sender, read_receiver) = channel();
        app.repository = Some(Repository {
            repo_source,
            side_panel,
            selected,
            side_panel_open: settings.side_panel_open,
            watcher,
            read_sender,
            read_receiver,
            generation: 0,
        });
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
        if let Some(change) = repo.watcher.as_ref().and_then(|watcher| watcher.take()) {
            let job = repo.repo_source.changed_on_disk(change);
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
            Applied::Failed { selected, brief } => {
                if let Some(commits) = &mut repo.side_panel.commits {
                    commits.set_selected(selected);
                }
                self.show_brief(brief);
            }
            Applied::Done {
                branch_name,
                commits,
                selected,
                changed_files,
                file,
                brief,
            } => {
                if let Some(list) = &mut repo.side_panel.commits {
                    list.set_commits(branch_name, commits, selected);
                }
                if let Some(changed_files) = changed_files {
                    repo.side_panel.files.set_changed_files(changed_files);
                }
                self.select(file);
                if let Some(brief) = brief {
                    self.show_brief(brief);
                }
            }
        }
    }

    fn show_brief(&mut self, brief: Notification) {
        self.notification_slot.show_brief(brief, Instant::now());
    }

    fn select(&mut self, file: Option<usize>) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        repo.selected = file;
        if let Some(file) = file {
            repo.side_panel.reveal_file(file);
        }
        let show = repo.repo_source.load(file);
        self.show(show);
    }

    fn show(&mut self, show: Show) {
        match show {
            Show::Unchanged => {}
            Show::Empty { pinned } => {
                self.notification_slot.pin(Some(pinned));
                self.diff.set_diff(empty_diff(), Default::default());
            }
            Show::Diff(shown) => {
                let Shown {
                    diff,
                    pinned,
                    paths,
                    can_rollback,
                    same_file,
                } = *shown;
                self.notification_slot.pin(pinned);
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
            Some(Effect::ShowBrief(text)) => self.show_brief(Notification::info(text)),
            Some(Effect::Copy(cursor_location)) => {
                clipboard::copy(&cursor_location, &mut io::stdout())?;
                self.show_brief(Notification::info(format!("Copied {cursor_location}")));
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
                    Some(Err(text)) => self.show_brief(Notification::warning(text)),
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
            Focus::SidePanel(section) => {
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
            AppCommand::ToggleDiffMode => {
                let effect = self.diff.toggle_diff_mode();
                self.effect(effect)?;
            }
            AppCommand::ZoomIn | AppCommand::ZoomOut | AppCommand::ResetZoom => {}
            AppCommand::NextFile | AppCommand::PrevFile => {
                let forward = cmd == AppCommand::NextFile;
                if let Some(repo) = &self.repository
                    && let Some(file) = repo.selected
                    && let Some(next) = repo.side_panel.files.next_file(file, forward)
                {
                    self.select(Some(next));
                }
            }
            AppCommand::ToggleSidePanel => {
                if let Some(repo) = &mut self.repository {
                    repo.side_panel_open = !repo.side_panel_open;
                    if !repo.side_panel_open {
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
                    self.focus = Focus::SidePanel(section);
                }
            }
            AppCommand::FocusRight => self.focus = Focus::Diff,
            AppCommand::FocusDown | AppCommand::FocusUp => {
                let (from, to) = match cmd {
                    AppCommand::FocusDown => (Section::Files, Section::Commits),
                    _ => (Section::Commits, Section::Files),
                };
                if self.focus == Focus::SidePanel(from) && self.section_open(to) {
                    self.focus = Focus::SidePanel(to);
                }
            }
        }
        Ok(true)
    }

    fn section_open(&self, section: Section) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|repo| repo.side_panel_open && repo.side_panel.is_open(section))
    }

    fn list_key(&mut self, section: Section, key: jetdiff_viewmodel::Key) {
        let Some(repo) = &mut self.repository else {
            return;
        };
        match list_key(key) {
            Some(ListKey::Down) => repo.side_panel.move_cursor(section, true),
            Some(ListKey::Up) => repo.side_panel.move_cursor(section, false),
            Some(ListKey::Select) => match section {
                Section::Files => {
                    if let Some(file) = repo.side_panel.files.select_at_cursor() {
                        self.select(Some(file));
                    }
                }
                Section::Commits => {
                    let selected = repo
                        .side_panel
                        .commits
                        .as_mut()
                        .and_then(|commits| commits.select_at_cursor());
                    self.select_in_commit_list(selected);
                }
            },
            None => {}
        }
    }

    fn select_in_commit_list(&mut self, selected: Option<Selected>) {
        if let (Some(selected), Some(repo)) = (selected, &mut self.repository) {
            let job = repo.repo_source.select(selected);
            repo.read(job);
        }
    }

    fn side_panel_contains(&self, x: u16, y: u16) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|repo| repo.side_panel_open)
            && self.side_panel_area.contains((x, y).into())
    }

    fn click(&mut self, x: u16, y: u16) {
        if !self.side_panel_contains(x, y) {
            self.focus = Focus::Diff;
            self.diff.click(x, y);
            return;
        }
        let Some(repo) = &mut self.repository else {
            return;
        };
        match repo.side_panel.click(y) {
            Some(Click::Heading(section)) => {
                repo.side_panel.toggle(section);
                if self.focus == Focus::SidePanel(section) && !repo.side_panel.is_open(section) {
                    self.focus = Focus::Diff;
                }
            }
            Some(Click::File(row)) => {
                self.focus = Focus::SidePanel(Section::Files);
                if let Some(file) = repo.side_panel.files.select(row) {
                    self.select(Some(file));
                }
            }
            Some(Click::Commit(row)) => {
                self.focus = Focus::SidePanel(Section::Commits);
                let selected = repo
                    .side_panel
                    .commits
                    .as_mut()
                    .and_then(|commits| commits.select(row));
                self.select_in_commit_list(selected);
            }
            None => {}
        }
    }

    fn wheel(&mut self, x: u16, y: u16, down: bool) {
        if self.side_panel_contains(x, y) {
            if let Some(repo) = &mut self.repository {
                repo.side_panel.wheel(y, down);
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
        if let Some(repo) = self.repository.as_mut().filter(|repo| repo.side_panel_open) {
            let width = repo.side_panel.width(area.width);
            self.side_panel_area = Rect {
                width: width.min(area.width),
                ..main
            };
            let section = match focus {
                Focus::SidePanel(section) => Some(section),
                Focus::Diff => None,
            };
            repo.side_panel
                .render(self.side_panel_area, section, repo.selected, &theme, buf);
            main.x += self.side_panel_area.width;
            main.width -= self.side_panel_area.width;
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
        widgets::keymap_bar(footer, &theme, buf);
        if let Some(notification) = self
            .notification_slot
            .current(Instant::now())
            .filter(|_| main.height > 0)
        {
            let row = Rect {
                y: main.bottom() - 1,
                height: 1,
                ..main
            };
            widgets::notification(notification, row, &theme, buf);
        }
        match &self.diff.overlay {
            Some(Overlay::Help) => widgets::help(area, &theme, buf),
            Some(Overlay::Confirm(_)) => widgets::confirm_rollback(main, &theme, buf),
            None => {}
        }
    }
}
