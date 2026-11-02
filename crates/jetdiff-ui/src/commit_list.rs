//! The commit list under the file tree: Branch, with Local changes and the commits under it. A click on
//! a row shows it.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, Div, EventEmitter, FocusHandle, Focusable, Hsla, ImageSource, KeyDownEvent,
    Render, Resource, ScrollStrategy, SharedString, UniformListScrollHandle, Window, div, img,
    prelude::*, px, relative, uniform_list,
};
use jetdiff_app::{CommitRow, Commits, LOCAL_CHANGES, Selected, age};
use jetdiff_git::Commit;

use crate::icons::branch_icon;
use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, TEXT_SIZE, list_key, row_bg};
use crate::settings::Settings;
use crate::text::FAMILY;
use crate::theme::UiTheme;

/// Room for the message with the age under it.
const COMMIT_ROW_HEIGHT: f32 = 40.;
const AGE_SIZE: f32 = 11.;
const LANE_WIDTH: f32 = 16.;
const LINE_WIDTH: f32 = 2.;
const DOT_SIZE: f32 = 8.;
/// Per level, so the slices sit one lane in under Branch.
const INDENT: f32 = LANE_WIDTH;

pub struct SelectRow(pub Selected);

pub struct CommitList {
    commits: Commits,
    scroll: UniformListScrollHandle,
    scale: f32,
    focus: FocusHandle,
}

impl EventEmitter<SelectRow> for CommitList {}

impl CommitList {
    pub fn new(commits: Commits, cx: &mut Context<Self>) -> Self {
        Self {
            commits,
            scroll: UniformListScrollHandle::new(),
            scale: 1.,
            focus: cx.focus_handle(),
        }
    }

    pub fn set_commits(
        &mut self,
        branch_name: Option<String>,
        commits: Vec<Commit>,
        selected: Selected,
        cx: &mut Context<Self>,
    ) {
        self.commits.set_commits(branch_name, commits, selected);
        cx.notify();
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    pub fn describe_selected(&self) -> String {
        self.commits.describe_selected()
    }

    /// Without `SelectRow`, for when the view could not switch.
    pub fn set_selected(&mut self, selected: Selected, cx: &mut Context<Self>) {
        self.commits.set_selected(selected);
        cx.notify();
    }

    /// Selects the row next to the selected one, also without focus.
    pub fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        let row = self.commits.row_next_to_selected(down);
        self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        let selected = self.commits.select(row);
        self.selected(selected, cx);
    }

    fn selected(&mut self, selected: Option<Selected>, cx: &mut Context<Self>) {
        if let Some(selected) = selected {
            cx.emit(SelectRow(selected));
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
            ListKey::Select => {
                let selected = self.commits.select_at_cursor();
                return self.selected(selected, cx);
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
        let (text, age) = match self.commits.row(i) {
            CommitRow::Branch(label) => (SharedString::from(label.to_owned()), None),
            CommitRow::LocalChanges => (LOCAL_CHANGES.into(), None),
            CommitRow::Commit(commit) => (
                SharedString::from(commit.subject.clone()),
                Some(age(now - commit.time)),
            ),
        };
        let indent = self.commits.indent(i) as f32 * INDENT;
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
            .pl(s(PAD_LEFT + indent))
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
                let selected = this.commits.select(i);
                this.selected(selected, cx)
            }))
    }

    /// The branch icon on Branch. Under it one line through the slices, with a dot per commit, and a
    /// ring for Local changes. `bg` is the row's background, so the ring covers the line without
    /// standing out.
    fn graph(&self, i: usize, bg: Option<Hsla>, theme: &UiTheme) -> Div {
        let s = |v: f32| px(v * self.scale);
        let lane = div().relative().flex_none().w(s(LANE_WIDTH)).h_full();
        let row = self.commits.row(i);
        if let CommitRow::Branch(_) = row {
            // Explicitly Embedded, otherwise GPUI reads `branch.svg` as a URI and fetches it over HTTP
            let icon = ImageSource::Resource(Resource::Embedded(branch_icon(theme.dark).into()));
            return lane
                .flex()
                .items_center()
                .child(img(icon).size(s(LANE_WIDTH)));
        }
        let color = theme.commit_graph;
        let line = || {
            div()
                .absolute()
                .left(s((LANE_WIDTH - LINE_WIDTH) / 2.))
                .w(s(LINE_WIDTH))
                .h(relative(0.5))
                .bg(color)
        };
        let is_commit = matches!(row, CommitRow::Commit(_));
        let dot = div()
            .absolute()
            .left(s((LANE_WIDTH - DOT_SIZE) / 2.))
            .top(s((COMMIT_ROW_HEIGHT - DOT_SIZE) / 2.))
            .size(s(DOT_SIZE))
            .rounded_full()
            .map(|d| {
                if is_commit {
                    d.bg(color)
                } else {
                    d.border(s(LINE_WIDTH))
                        .border_color(color)
                        // Covers the line below, in the same color as the row
                        .bg(bg.unwrap_or(theme.editor_bg))
                }
            });
        lane.when(is_commit, |d| d.child(line().top_0()))
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
