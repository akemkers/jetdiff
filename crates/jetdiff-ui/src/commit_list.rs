//! Commitene under filtreet. Første rad er working tree, klikk på en rad viser den.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, Div, EventEmitter, FocusHandle, Focusable, Hsla, KeyDownEvent, Render,
    ScrollStrategy, SharedString, UniformListScrollHandle, Window, div, prelude::*, px, relative,
    uniform_list,
};
use jetdiff_app::{Commits, LOCAL_CHANGES, age};
use jetdiff_git::Commit;

use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, TEXT_SIZE, list_key, row_bg};
use crate::settings::Settings;
use crate::text::FAMILY;
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
    commits: Commits,
    scroll: UniformListScrollHandle,
    scale: f32,
    focus: FocusHandle,
}

impl EventEmitter<SelectCommit> for CommitList {}

impl CommitList {
    pub fn new(commits: Vec<Commit>, cx: &mut Context<Self>) -> Self {
        Self {
            commits: Commits::new(commits),
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            focus: cx.focus_handle(),
        }
    }

    pub fn set_commits(&mut self, commits: Vec<Commit>, cx: &mut Context<Self>) {
        self.commits.set_commits(commits);
        cx.notify();
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    /// Uten `SelectCommit`, for når visningen ikke fikk byttet.
    pub fn set_selected(&mut self, hash: Option<String>, cx: &mut Context<Self>) {
        self.commits.set_selected(hash);
        cx.notify();
    }

    fn selected(&mut self, hash: Option<Option<String>>, cx: &mut Context<Self>) {
        if let Some(hash) = hash {
            cx.emit(SelectCommit(hash));
        }
        cx.notify();
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = list_key(ev) else {
            return;
        };
        cx.stop_propagation();
        let row = match key {
            ListKey::Down => self.commits.move_cursor(true),
            ListKey::Up => self.commits.move_cursor(false),
            ListKey::Open => {
                let hash = self.commits.open_cursor();
                return self.selected(hash, cx);
            }
        };
        self.scroll.scroll_to_item(row, ScrollStrategy::Top);
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
        let commit: Option<&Commit> = self.commits.commit(i);
        let (text, age) = match commit {
            Some(c) => (
                SharedString::from(c.subject.clone()),
                Some(age(now - c.time)),
            ),
            None => (LOCAL_CHANGES.into(), None),
        };
        let theme = cx.global::<Settings>().theme;
        let bg = row_bg(
            &theme,
            self.commits.is_cursor(i),
            self.commits.is_selected(i),
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
            .child(self.graph(i, bg, &theme))
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
                                .text_color(theme.line_number)
                                .child(age),
                        )
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let hash = this.commits.select(i);
                this.selected(hash, cx)
            }))
    }

    /// Én linje gjennom alle radene, med en prikk per commit. Working tree er en ring.
    /// `bg` er bakgrunnen til raden, så ringen dekker linja uten å skille seg ut.
    fn graph(&self, i: usize, bg: Option<Hsla>, theme: &UiTheme) -> Div {
        let s = |v: f32| px(v * self.scale);
        let color = theme.commit_graph;
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
                    .bg(bg.unwrap_or(theme.editor_bg)),
                _ => d.bg(color),
            });
        div()
            .relative()
            .flex_none()
            .w(s(LANE_WIDTH))
            .h_full()
            .when(i > 0, |d| d.child(line().top_0()))
            .when(i + 1 < self.commits.len(), |d| d.child(line().bottom_0()))
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
            .font_family(FAMILY)
            .text_size(s(TEXT_SIZE))
            .text_color(cx.global::<Settings>().theme.editor_fg)
            .child(
                uniform_list(
                    "commits",
                    self.commits.len(),
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
