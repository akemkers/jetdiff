//! Commitene under filtreet. Første rad er working tree, klikk på en rad viser den.

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    Context, Div, EventEmitter, Render, SharedString, UniformListScrollHandle, Window, div,
    prelude::*, px, relative, uniform_list,
};
use smudiff_git::Commit;
use smudiff_view::metrics::FONT_SIZE;

use crate::changes_tree::{GAP, PAD_LEFT, PAD_RIGHT, ROW_HEIGHT, TEXT_SIZE};
use crate::theme::UiTheme;

/// Litt luftigere enn treet, så grafen får plass.
const COMMIT_ROW_HEIGHT: f32 = ROW_HEIGHT + 4.;
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
    theme: UiTheme,
    family: &'static str,
    scroll: UniformListScrollHandle,
    scale: f32,
}

impl EventEmitter<SelectCommit> for CommitList {}

impl CommitList {
    pub fn new(label: String, commits: Vec<Commit>, theme: UiTheme, family: &'static str) -> Self {
        Self {
            label: label.into(),
            commits: commits.into(),
            selected: None,
            theme,
            family,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
        }
    }

    pub fn set_commits(&mut self, label: String, commits: Vec<Commit>, cx: &mut Context<Self>) {
        self.label = label.into();
        self.commits = commits.into();
        cx.notify();
    }

    pub fn set_font_size(&mut self, font_size: f32, cx: &mut Context<Self>) {
        self.scale = font_size / FONT_SIZE;
        cx.notify();
    }

    fn select(&mut self, hash: Option<String>, cx: &mut Context<Self>) {
        if hash != self.selected {
            self.selected = hash.clone();
            cx.emit(SelectCommit(hash));
            cx.notify();
        }
    }

    fn render_row(&self, i: usize, now: i64, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let s = |v: f32| px(v * self.scale);
        let commit = i.checked_sub(1).map(|c| &self.commits[c]);
        let hash = commit.map(|c| c.hash.clone());
        let (text, age) = match commit {
            Some(c) => (SharedString::from(c.subject.clone()), age(now - c.time)),
            None => (self.label.clone(), String::new()),
        };
        div()
            .id(i)
            .h(s(COMMIT_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(s(GAP * 2.))
            .pl(s(PAD_LEFT))
            .pr(s(PAD_RIGHT))
            .whitespace_nowrap()
            .when(hash == self.selected, |d| d.bg(self.theme.tree_selection))
            .child(self.graph(i, hash == self.selected))
            .child(div().flex_1().min_w_0().truncate().child(text))
            .child(
                div()
                    .flex_none()
                    .text_color(self.theme.line_number)
                    .child(age),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.select(hash.clone(), cx)))
    }

    /// Én linje gjennom alle radene, med en prikk per commit. Working tree er en ring.
    fn graph(&self, i: usize, selected: bool) -> Div {
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
                    .bg(if selected {
                        self.theme.tree_selection
                    } else {
                        self.theme.editor_bg
                    }),
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

impl Render for CommitList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = |v: f32| px(v * self.scale);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(self.family)
            .text_size(s(TEXT_SIZE))
            .text_color(self.theme.editor_fg)
            .child(
                div()
                    .flex_none()
                    .h(s(ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .pl(s(PAD_LEFT))
                    .text_color(self.theme.line_number)
                    .child("Commits"),
            )
            .child(
                uniform_list(
                    "commits",
                    self.commits.len() + 1,
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| this.render_row(i, now, cx))
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
