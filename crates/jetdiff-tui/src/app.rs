//! The event loop: keys and mouse, the watcher and reading the repo source in the background.

mod source;

use std::io;
use std::time::{Duration, Instant};

use jetdiff_app::watcher::POLL;
use jetdiff_app::{
    AppCommand, Applied, Commits, FilePair, Notification, NotificationSlot, Overlay, RepoSource,
    Selected, Show, Shown, SlotContent, app_command, empty_diff,
};
use jetdiff_viewmodel::{FileDiff, Theme};
use ratatui::Frame;
use ratatui::crossterm::event::{
    self, Event, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::Rect;

use crate::clipboard;
use crate::diff_view::{DiffView, Effect};
use crate::input::to_key;
use crate::side_panel::{Click, ListKey, Section, list_key};
use crate::widgets;
use crate::{Settings, Term};
use source::{Repository, Source};

/// How often smooth scroll checks the row. A frame is drawn only when the row has changed.
const ANIMATION_TICK: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Diff,
    SidePanel(Section),
}

pub struct App {
    theme: Theme,
    diff: DiffView,
    notification_slot: NotificationSlot,
    source: Source,
    focus: Focus,
    side_panel_area: Rect,
    quit: bool,
}

impl App {
    pub fn file_pair(file_pair: FilePair, diff: FileDiff, settings: &Settings) -> Self {
        let paths = file_pair.paths().clone();
        Self::new(diff, paths, Source::FilePair(file_pair), settings)
    }

    pub fn repo_source(
        repo_source: RepoSource,
        commits: Option<Commits>,
        settings: &Settings,
    ) -> Self {
        let (repository, warning) = Repository::new(repo_source, commits, settings);
        let selected = repository.selected();
        let source = Source::RepoSource(Box::new(repository));
        let mut app = Self::new(empty_diff(), Default::default(), source, settings);
        if let Some(warning) = warning {
            app.show_brief(warning);
        }
        app.select(selected);
        app
    }

    fn new(diff: FileDiff, paths: [String; 2], source: Source, settings: &Settings) -> Self {
        Self {
            theme: settings.theme,
            diff: DiffView::new(
                diff,
                paths,
                settings.unified,
                settings.fold_unchanged,
                settings.smooth_scroll,
            ),
            notification_slot: NotificationSlot::default(),
            source,
            focus: Focus::Diff,
            side_panel_area: Rect::default(),
            quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut Term) -> io::Result<()> {
        while !self.quit {
            queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
            terminal.draw(|frame| self.draw(frame))?;
            execute!(terminal.backend_mut(), EndSynchronizedUpdate)?;
            self.wait()?;
            self.background();
        }
        Ok(())
    }

    /// Handles the events that come within `POLL`. During smooth scroll it returns as soon as the
    /// row shown changes, so a frame is drawn per row and not per tick.
    fn wait(&mut self) -> io::Result<()> {
        let deadline = Instant::now() + POLL;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout = if self.diff.is_animating() {
                remaining.min(ANIMATION_TICK)
            } else {
                remaining
            };
            let handled = event::poll(timeout)? && self.handle_events()?;
            let now = Instant::now();
            let moved = self.diff.step_animation(now);
            if handled || moved || now >= deadline {
                return Ok(());
            }
        }
    }

    /// All the events that are waiting. `false` when there was only mouse motion, which changes nothing.
    fn handle_events(&mut self) -> io::Result<bool> {
        let mut handled = false;
        while event::poll(Duration::ZERO)? {
            let event = event::read()?;
            handled |= !matches!(
                event,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Moved,
                    ..
                })
            );
            self.event(event)?;
        }
        Ok(handled)
    }

    fn background(&mut self) {
        if let Some(applied) = self
            .source
            .repository_mut()
            .and_then(|repository| repository.background())
        {
            self.show_applied(applied);
        }
    }

    fn show_applied(&mut self, applied: Applied) {
        let Some(repo) = self.source.repository_mut() else {
            return;
        };
        let side_panel = repo.side_panel_mut();
        match applied {
            Applied::Failed { selected, brief } => {
                if let Some(commits) = &mut side_panel.commits {
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
                if let Some(list) = &mut side_panel.commits {
                    list.set_commits(branch_name, commits, selected);
                }
                if let Some(changed_files) = changed_files {
                    side_panel.files.set_changed_files(changed_files);
                }
                self.select(file);
                if let Some(brief) = brief {
                    self.show_brief(brief);
                }
            }
        }
    }

    /// A repo source is read in the background, a File pair right away. The cursor and scroll are
    /// kept for the same file.
    fn source_reload(&mut self) {
        match &mut self.source {
            Source::RepoSource(repository) => repository.source_reload(),
            Source::FilePair(file_pair) => {
                let (diff, brief) = file_pair.source_reload();
                if let Some(diff) = diff {
                    self.diff.update_diff(diff, file_pair.paths().clone());
                }
                self.notification_slot.show_brief(brief, Instant::now());
            }
        }
    }

    fn show_brief(&mut self, brief: Notification) {
        self.notification_slot.show_brief(brief, Instant::now());
    }

    fn select(&mut self, file: Option<usize>) {
        if let Some(show) = self
            .source
            .repository_mut()
            .map(|repository| repository.load(file))
        {
            self.show(show);
        }
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
                let Some(repo) = self.source.repository_mut() else {
                    return Ok(());
                };
                match repo.rollback(&request) {
                    None => {}
                    Some(Ok(show)) => self.show(show),
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
                    MouseEventKind::Drag(MouseButton::Left)
                        if self.diff.is_dragging_diff_divider() =>
                    {
                        self.diff.drag_diff_divider(m.column)
                    }
                    MouseEventKind::Up(MouseButton::Left) => self.diff.release_diff_divider(),
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
            AppCommand::SourceReload => self.source_reload(),
            AppCommand::NextFile | AppCommand::PrevFile => {
                let forward = cmd == AppCommand::NextFile;
                if let Some(repo) = self.source.repository()
                    && let Some(file) = repo.selected()
                    && let Some(next) = repo.side_panel().files.next_file(file, forward)
                {
                    self.select(Some(next));
                }
            }
            // The walkthrough is only in the window so far
            AppCommand::ToggleWalkthrough => return Ok(false),
            AppCommand::NextCommit | AppCommand::PrevCommit => {
                let selected = self.source.repository_mut().and_then(|repo| {
                    repo.side_panel_mut()
                        .step_commit(cmd == AppCommand::NextCommit)
                });
                self.select_in_commit_list(selected);
            }
            AppCommand::ToggleSidePanel => {
                if let Some(repo) = self.source.repository_mut()
                    && !repo.toggle_side_panel()
                {
                    self.focus = Focus::Diff;
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
        self.source
            .repository()
            .is_some_and(|repo| repo.side_panel_open() && repo.side_panel().is_open(section))
    }

    fn list_key(&mut self, section: Section, key: jetdiff_viewmodel::Key) {
        let Some(repo) = self.source.repository_mut() else {
            return;
        };
        let side_panel = repo.side_panel_mut();
        match list_key(key) {
            Some(ListKey::Down) => side_panel.move_cursor(section, true),
            Some(ListKey::Up) => side_panel.move_cursor(section, false),
            Some(ListKey::Select) => match section {
                Section::Files => {
                    if let Some(file) = side_panel.files.select_at_cursor() {
                        self.select(Some(file));
                    }
                }
                Section::Commits => {
                    let selected = side_panel
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
        if let (Some(selected), Some(repo)) = (selected, self.source.repository_mut()) {
            repo.select_commit(selected);
        }
    }

    fn side_panel_contains(&self, x: u16, y: u16) -> bool {
        self.source
            .repository()
            .is_some_and(|repo| repo.side_panel_open())
            && self.side_panel_area.contains((x, y).into())
    }

    fn click(&mut self, x: u16, y: u16) {
        if !self.side_panel_contains(x, y) {
            self.focus = Focus::Diff;
            self.diff.click(x, y, Instant::now());
            return;
        }
        self.diff.press_elsewhere();
        let Some(repo) = self.source.repository_mut() else {
            return;
        };
        let side_panel = repo.side_panel_mut();
        match side_panel.click(y) {
            Some(Click::Heading(section)) => {
                side_panel.toggle(section);
                if self.focus == Focus::SidePanel(section) && !side_panel.is_open(section) {
                    self.focus = Focus::Diff;
                }
            }
            Some(Click::File(row)) => {
                self.focus = Focus::SidePanel(Section::Files);
                if let Some(file) = side_panel.files.select(row) {
                    self.select(Some(file));
                }
            }
            Some(Click::Commit(row)) => {
                self.focus = Focus::SidePanel(Section::Commits);
                let selected = side_panel
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
            if let Some(repo) = self.source.repository_mut() {
                repo.side_panel_mut().wheel(y, down);
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
        if let Some(repo) = self
            .source
            .repository_mut()
            .filter(|repo| repo.side_panel_open())
        {
            let selected = repo.selected();
            let side_panel = repo.side_panel_mut();
            let width = side_panel.width(area.width);
            self.side_panel_area = Rect {
                width: width.min(area.width),
                ..main
            };
            let section = match focus {
                Focus::SidePanel(section) => Some(section),
                Focus::Diff => None,
            };
            side_panel.render(self.side_panel_area, section, selected, &theme, buf);
            main.x += self.side_panel_area.width;
            main.width -= self.side_panel_area.width;
        }
        if let Some(file) = self
            .source
            .repository()
            .and_then(|repository| repository.shown())
        {
            widgets::header(&file.path, Rect { height: 1, ..main }, &theme, buf);
            main.y += 1;
            main.height = main.height.saturating_sub(1);
        }
        self.diff.layout(main);
        self.diff.render(focus == Focus::Diff, &theme, buf);
        widgets::keymap_bar(footer, &theme, buf);
        let now = Instant::now();
        if let Some(content) = self
            .notification_slot
            .current(now)
            .filter(|_| main.height > 0)
        {
            let row = Rect {
                y: main.bottom() - 1,
                height: 1,
                ..main
            };
            let notification = match content {
                SlotContent::Notification(notification) => notification.clone(),
                SlotContent::Progress(progress) => {
                    Notification::info(format!("{} {}", progress.text, progress.elapsed(now)))
                }
            };
            widgets::notification(&notification, row, &theme, buf);
        }
        match self.diff.overlay() {
            Some(Overlay::Help) => widgets::help(area, &theme, buf),
            Some(Overlay::Rollback(_)) => widgets::confirm_rollback(main, &theme, buf),
            None => {}
        }
    }
}
