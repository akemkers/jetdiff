//! Filtreet og commit-lista til venstre.

use std::time::{SystemTime, UNIX_EPOCH};

use jetdiff_app::{Commits, FileTree, LOCAL_CHANGES, age, count_label, file_name};
use jetdiff_changes::{RowKind, Status};
use jetdiff_viewmodel::columns::display_width;
use jetdiff_viewmodel::{Key, KeyCode, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::color::color;
use crate::icons::{Icon, file_icon, folder_icon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Files,
    Commits,
}

pub enum ListKey {
    Down,
    Up,
    Open,
}

pub fn list_key(key: Key) -> Option<ListKey> {
    if key.ctrl || key.cmd {
        return None;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(ListKey::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(ListKey::Up),
        KeyCode::Char('l') | KeyCode::Enter => Some(ListKey::Open),
        _ => None,
    }
}

#[derive(Default, Clone, Copy)]
struct ListArea {
    area: Rect,
    scroll: usize,
}

impl ListArea {
    fn reveal(&mut self, row: usize) {
        let height = self.area.height as usize;
        if row < self.scroll {
            self.scroll = row;
        } else if height > 0 && row >= self.scroll + height {
            self.scroll = row + 1 - height;
        }
    }

    fn row_at(&self, y: u16) -> Option<usize> {
        let r = self.area;
        (y >= r.y && y < r.bottom()).then(|| self.scroll + (y - r.y) as usize)
    }
}

pub struct Panel {
    pub tree: FileTree,
    pub commits: Option<Commits>,
    pub files_open: bool,
    pub commits_open: bool,
    icons: bool,
    files: ListArea,
    commit_rows: ListArea,
    headings: [Option<u16>; 2],
}

pub enum Click {
    Heading(Section),
    File(usize),
    Commit(usize),
}

impl Panel {
    pub fn new(tree: FileTree, commits: Option<Commits>, icons: bool) -> Self {
        Self {
            tree,
            commits,
            files_open: true,
            commits_open: true,
            icons,
            files: ListArea::default(),
            commit_rows: ListArea::default(),
            headings: [None; 2],
        }
    }

    pub fn is_open(&self, section: Section) -> bool {
        match section {
            Section::Files => self.files_open,
            Section::Commits => self.commits.is_some() && self.commits_open,
        }
    }

    pub fn toggle(&mut self, section: Section) {
        match section {
            Section::Files => self.files_open = !self.files_open,
            Section::Commits => self.commits_open = !self.commits_open,
        }
    }

    pub fn reveal_file(&mut self, file: usize) {
        if let Some(row) = self.tree.reveal(file) {
            self.files.reveal(row);
        }
    }

    pub fn move_cursor(&mut self, section: Section, down: bool) {
        match section {
            Section::Files => {
                let row = self.tree.move_cursor(down);
                self.files.reveal(row);
            }
            Section::Commits => {
                if let Some(commits) = &mut self.commits {
                    let row = commits.move_cursor(down);
                    self.commit_rows.reveal(row);
                }
            }
        }
    }

    pub fn width(&self, max: u16) -> u16 {
        let changes = self.tree.changes();
        let icon_width = if self.icons { 2 } else { 0 };
        let widest = self
            .tree
            .rows()
            .iter()
            .map(|row| {
                2 * row.depth
                    + 2
                    + icon_width
                    + match &row.kind {
                        RowKind::Dir { name, count, .. } => {
                            display_width(name) + 1 + count_label(*count).len()
                        }
                        &RowKind::File(f) => {
                            let change = &changes[f];
                            let from = change.renamed_from().map_or(0, |t| 1 + display_width(&t));
                            display_width(file_name(&change.path)) + from
                        }
                    }
            })
            .max()
            .unwrap_or(0);
        (widest as u16 + 2).clamp(24, max.max(24))
    }

    pub fn click(&self, y: u16) -> Option<Click> {
        for (heading, section) in self.headings.iter().zip([Section::Files, Section::Commits]) {
            if *heading == Some(y) {
                return Some(Click::Heading(section));
            }
        }
        if let Some(row) = self.files.row_at(y).filter(|&r| r < self.tree.rows().len()) {
            return Some(Click::File(row));
        }
        let commits = self.commits.as_ref()?;
        self.commit_rows
            .row_at(y)
            .filter(|&r| r < commits.len())
            .map(Click::Commit)
    }

    pub fn wheel(&mut self, y: u16, down: bool) {
        for (list, len) in [
            (&mut self.files, self.tree.rows().len()),
            (
                &mut self.commit_rows,
                self.commits.as_ref().map_or(0, |commits| commits.len()),
            ),
        ] {
            if (list.area.y..list.area.bottom()).contains(&y) {
                let max = len.saturating_sub(list.area.height as usize);
                list.scroll = if down {
                    (list.scroll + 3).min(max)
                } else {
                    list.scroll.saturating_sub(3)
                };
            }
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        focus: Option<Section>,
        selected: Option<usize>,
        theme: &Theme,
        buf: &mut Buffer,
    ) {
        let bg = color(theme.editor_bg);
        buf.set_style(area, Style::default().bg(bg).fg(color(theme.editor_fg)));
        let border = Rect {
            x: area.right() - 1,
            width: 1,
            ..area
        };
        for y in border.top()..border.bottom() {
            buf[(border.x, y)]
                .set_char('│')
                .set_fg(color(theme.indent_guide));
        }
        let area = Rect {
            width: area.width - 1,
            ..area
        };
        self.headings = [None; 2];
        self.files.area = Rect::default();
        self.commit_rows.area = Rect::default();

        let has_commits = self.commits.is_some();
        let mut y = area.y;
        self.heading(Section::Files, area, y, theme, buf);
        y += 1;
        let commits_top = match (self.files_open, has_commits && self.commits_open) {
            (_, false) if has_commits => area.bottom() - 1,
            (_, false) => area.bottom(),
            (true, true) => area.bottom() - (area.height * 2 / 5).max(3),
            (false, true) => y,
        };
        if self.files_open {
            self.files.area = Rect {
                y,
                height: commits_top.saturating_sub(y),
                ..area
            };
            self.render_files(focus == Some(Section::Files), selected, theme, buf);
        }
        if has_commits {
            self.heading(Section::Commits, area, commits_top, theme, buf);
            if self.commits_open {
                let top = commits_top + 1;
                self.commit_rows.area = Rect {
                    y: top,
                    height: area.bottom().saturating_sub(top),
                    ..area
                };
                self.render_commits(focus == Some(Section::Commits), theme, buf);
            }
        }
    }

    fn heading(&mut self, section: Section, area: Rect, y: u16, theme: &Theme, buf: &mut Buffer) {
        if y >= area.bottom() {
            return;
        }
        let (title, open) = match section {
            Section::Files => ("Filer", self.files_open),
            Section::Commits => ("Commits", self.commits_open),
        };
        let text = format!(" {title} {}", if open { "−" } else { "+" });
        let style = Style::default().fg(color(theme.line_number));
        buf.set_stringn(area.x, y, text, area.width as usize, style);
        self.headings[section as usize] = Some(y);
    }

    fn row_bg(
        theme: &Theme,
        cursor: bool,
        open: bool,
        focused: bool,
    ) -> Option<ratatui::style::Color> {
        if focused && cursor {
            Some(color(theme.tree_selection))
        } else if open {
            Some(color(theme.tree_selection_inactive))
        } else {
            None
        }
    }

    fn render_files(
        &self,
        focused: bool,
        selected: Option<usize>,
        theme: &Theme,
        buf: &mut Buffer,
    ) {
        let list = self.files;
        let rows = self.tree.rows();
        let changes = self.tree.changes();
        let dim = Style::default().fg(color(theme.line_number));
        for (i, row) in rows
            .iter()
            .enumerate()
            .skip(list.scroll)
            .take(list.area.height as usize)
        {
            let y = list.area.y + (i - list.scroll) as u16;
            let indent = " ".repeat(1 + 2 * row.depth);
            let mut spans = vec![Span::raw(indent)];
            match &row.kind {
                RowKind::Dir {
                    name,
                    count,
                    expanded,
                    ..
                } => {
                    spans.push(Span::styled(if *expanded { "▾ " } else { "▸ " }, dim));
                    if self.icons {
                        spans.push(icon(folder_icon(theme.dark)));
                    }
                    spans.push(Span::raw(name.clone()));
                    spans.push(Span::styled(format!(" {}", count_label(*count)), dim));
                }
                &RowKind::File(f) => {
                    let change = &changes[f];
                    let status = match change.status {
                        Status::Added => theme.file_added,
                        Status::Deleted => theme.file_deleted,
                        Status::Modified | Status::Renamed => theme.file_modified,
                    };
                    let style = Style::default().fg(color(status));
                    spans.push(Span::raw("  "));
                    if self.icons {
                        spans.push(icon(file_icon(&change.path, theme.dark)));
                    }
                    spans.push(Span::styled(file_name(&change.path).to_owned(), style));
                    if let Some(from) = change.renamed_from() {
                        spans.push(Span::styled(format!(" {from}"), dim));
                    }
                }
            }
            let open = selected.is_some_and(|f| row.kind == RowKind::File(f));
            let line_area = Rect {
                y,
                height: 1,
                ..list.area
            };
            if let Some(bg) = Self::row_bg(theme, i == self.tree.cursor(), open, focused) {
                buf.set_style(line_area, Style::default().bg(bg));
            }
            buf.set_line(line_area.x, y, &Line::from(spans), line_area.width);
        }
    }

    fn render_commits(&self, focused: bool, theme: &Theme, buf: &mut Buffer) {
        let Some(commits) = &self.commits else {
            return;
        };
        let list = self.commit_rows;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let graph = Style::default().fg(color(theme.commit_graph));
        let dim = Style::default().fg(color(theme.line_number));
        for i in (list.scroll..commits.len()).take(list.area.height as usize) {
            let y = list.area.y + (i - list.scroll) as u16;
            let line_area = Rect {
                y,
                height: 1,
                ..list.area
            };
            let spans = match commits.commit(i) {
                Some(commit) => vec![
                    Span::styled(" ● ", graph),
                    Span::raw(commit.subject.clone()),
                    Span::styled(format!("  {}", age(now - commit.time)), dim),
                ],
                None => vec![Span::styled(" ○ ", graph), Span::raw(LOCAL_CHANGES)],
            };
            if let Some(bg) =
                Self::row_bg(theme, commits.is_cursor(i), commits.is_selected(i), focused)
            {
                buf.set_style(line_area, Style::default().bg(bg));
            }
            buf.set_line(line_area.x, y, &Line::from(spans), line_area.width);
        }
    }
}

fn icon(icon: Icon) -> Span<'static> {
    Span::styled(format!("{} ", icon.glyph), Style::default().fg(icon.color))
}
