//! Commitene under filtreet. Første rad er working tree, klikk på en rad viser den.

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, Div, EventEmitter, FocusHandle, Focusable, Hsla, KeyDownEvent, Render,
    ScrollStrategy, SharedString, UniformListScrollHandle, Window, div, prelude::*, px, relative,
    uniform_list,
};
use jetdiff_git::Commit;

use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, TEXT_SIZE, list_key, row_bg, step};
use crate::theme::UiTheme;

/// Plass til meldingen med alderen under.
const COMMIT_ROW_HEIGHT: f32 = 40.;
const AGE_SIZE: f32 = 11.;
const LANE_WIDTH: f32 = 16.;
const LINE_WIDTH: f32 = 2.;
const DOT_SIZE: f32 = 8.;

/// Hashen til valgt commit. `None` er working tree.
pub struct SelectCommit(pub Option<String>);

pub struct CommitList {
    /// Første rad, som «Endringer mot main».
    label: SharedString,
    commits: Rc<[Commit]>,
    selected: Option<String>,
    /// Raden j og k flytter, som hash. Enter åpner den.
    cursor: Option<String>,
    theme: UiTheme,
    font_family: &'static str,
    scroll: UniformListScrollHandle,
    scale: f32,
    focus: FocusHandle,
}

impl EventEmitter<SelectCommit> for CommitList {}

impl CommitList {
    pub fn new(
        label: String,
        commits: Vec<Commit>,
        theme: UiTheme,
        font_family: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            label: label.into(),
            commits: commits.into(),
            selected: None,
            cursor: None,
            theme,
            font_family,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            focus: cx.focus_handle(),
        }
    }

    pub fn set_commits(&mut self, label: String, commits: Vec<Commit>, cx: &mut Context<Self>) {
        self.label = label.into();
        self.commits = commits.into();
        // En commit som er borte, som etter en rebase, gir working tree
        if self.row_of(&self.cursor) == 0 {
            self.cursor = None;
        }
        cx.notify();
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    /// Uten `SelectCommit`, for når visningen ikke fikk byttet.
    pub fn set_selected(&mut self, hash: Option<String>, cx: &mut Context<Self>) {
        self.selected = hash;
        cx.notify();
    }

    fn select(&mut self, hash: Option<String>, cx: &mut Context<Self>) {
        self.cursor = hash.clone();
        if hash != self.selected {
            self.selected = hash.clone();
            cx.emit(SelectCommit(hash));
        }
        cx.notify();
    }

    /// Rad 0 er working tree. En commit som ikke er i lista gir også rad 0.
    fn row_of(&self, hash: &Option<String>) -> usize {
        hash.as_ref().map_or(0, |h| {
            self.commits
                .iter()
                .position(|c| &c.hash == h)
                .map_or(0, |c| c + 1)
        })
    }

    fn hash_at(&self, row: usize) -> Option<String> {
        row.checked_sub(1).map(|c| self.commits[c].hash.clone())
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = list_key(ev) else {
            return;
        };
        cx.stop_propagation();
        let row = self.row_of(&self.cursor);
        let row = match key {
            ListKey::Down => step(row, true, self.commits.len() + 1),
            ListKey::Up => step(row, false, self.commits.len() + 1),
            ListKey::Open => return self.select(self.hash_at(row), cx),
        };
        self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        self.cursor = self.hash_at(row);
        cx.notify();
    }

    fn render_row(
        &self,
        i: usize,
        now: i64,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let s = |v: f32| px(v * self.scale);
        let commit = i.checked_sub(1).map(|c| &self.commits[c]);
        let hash = self.hash_at(i);
        let (text, age) = match commit {
            Some(c) => (
                SharedString::from(c.subject.clone()),
                Some(age(now - c.time)),
            ),
            None => (self.label.clone(), None),
        };
        let bg = row_bg(
            &self.theme,
            hash == self.cursor,
            hash == self.selected,
            focused,
        );
        div()
            .id(i)
            .h(s(COMMIT_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(s(GAP * 2.))
            .pl(s(PAD_LEFT))
            .pr(s(PAD_RIGHT))
            .whitespace_nowrap()
            .when_some(bg, |d, bg| d.bg(bg))
            .child(self.graph(i, bg))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().child(text))
                    .when_some(age, |d, age| {
                        d.child(
                            div()
                                .text_size(s(AGE_SIZE))
                                .text_color(self.theme.line_number)
                                .child(age),
                        )
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.select(hash.clone(), cx)))
    }

    /// Én linje gjennom alle radene, med en prikk per commit. Working tree er en ring.
    /// `bg` er bakgrunnen til raden, så ringen dekker linja uten å skille seg ut.
    fn graph(&self, i: usize, bg: Option<Hsla>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let color = self.theme.commit_graph;
        let line = || {
            div()
                .absolute()
                .left(s((LANE_WIDTH - LINE_WIDTH) / 2.))
                .w(s(LINE_WIDTH))
                .h(relative(0.5))
                .bg(color)
        };
        let dot = div()
            .absolute()
            .left(s((LANE_WIDTH - DOT_SIZE) / 2.))
            .top(s((COMMIT_ROW_HEIGHT - DOT_SIZE) / 2.))
            .size(s(DOT_SIZE))
            .rounded_full()
            .map(|d| match i {
                0 => d
                    .border(s(LINE_WIDTH))
                    .border_color(color)
                    // Dekker linja under, i samme farge som raden
                    .bg(bg.unwrap_or(self.theme.editor_bg)),
                _ => d.bg(color),
            });
        div()
            .relative()
            .flex_none()
            .w(s(LANE_WIDTH))
            .h_full()
            .when(i > 0, |d| d.child(line().top_0()))
            .when(i < self.commits.len(), |d| d.child(line().bottom_0()))
            .child(dot)
    }
}

impl Focusable for CommitList {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CommitList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let s = |v: f32| px(v * self.scale);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .font_family(self.font_family)
            .text_size(s(TEXT_SIZE))
            .text_color(self.theme.editor_fg)
            .child(
                uniform_list(
                    "commits",
                    self.commits.len() + 1,
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| this.render_row(i, now, focused, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .track_scroll(self.scroll.clone()),
            )
    }
}

/// Sekunder siden, som «3 timer siden».
fn age(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let ago =
        |n: i64, one: &str, many: &str| format!("{n} {} siden", if n == 1 { one } else { many });
    match seconds {
        s if s < MINUTE => "nå".into(),
        s if s < HOUR => format!("{} min siden", s / MINUTE),
        s if s < DAY => ago(s / HOUR, "time", "timer"),
        s if s < 30 * DAY => ago(s / DAY, "dag", "dager"),
        s if s < 365 * DAY => ago(s / (30 * DAY), "måned", "måneder"),
        s => ago(s / (365 * DAY), "år", "år"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alder() {
        assert_eq!(age(-5), "nå");
        assert_eq!(age(59), "nå");
        assert_eq!(age(120), "2 min siden");
        assert_eq!(age(3600), "1 time siden");
        assert_eq!(age(5 * 3600), "5 timer siden");
        assert_eq!(age(86400), "1 dag siden");
        assert_eq!(age(40 * 86400), "1 måned siden");
        assert_eq!(age(800 * 86400), "2 år siden");
    }
}
