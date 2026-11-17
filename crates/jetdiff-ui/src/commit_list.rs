//! The commit list under the file tree: Branch, with Local changes and the commits under it. A click on
//! a row shows it.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, Div, EventEmitter, FocusHandle, Focusable, ImageSource, KeyDownEvent, Render,
    Resource, ScrollStrategy, SharedString, UniformListScrollHandle, Window, div, img, prelude::*,
    px, relative, uniform_list,
};
use jetdiff_app::{CommitRow, Commits, LOCAL_CHANGES, Selected, age};
use jetdiff_git::Commit;

use crate::icons::branch_icon;
use crate::list::{GAP, ListKey, PAD_LEFT, PAD_RIGHT, TEXT_SIZE, list_key, selection_and_cursor};
use crate::settings::Settings;
use crate::text::{FAMILY, SANS};
use crate::theme::UiTheme;

/// Room for the message with the age under it.
const COMMIT_ROW_HEIGHT: f32 = 40.;
const AGE_SIZE: f32 = 12.;
const LANE_WIDTH: f32 = 16.;
const LINE_WIDTH: f32 = 2.;
const DOT_SIZE: f32 = 8.;
/// The cursor's outline. The line reaches across it, so it stays connected between rows.
const CURSOR_OUTLINE: f32 = 1.;
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
        scale_factor: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let s = |v: f32| px(v * self.scale);
        let (text, age, font_family) = match self.commits.row(i) {
            CommitRow::Branch(label) => (SharedString::from(label.to_owned()), None, FAMILY),
            CommitRow::LocalChanges => (LOCAL_CHANGES.into(), None, SANS),
            CommitRow::Commit(commit) => (
                SharedString::from(commit.subject.clone()),
                Some(age(now - commit.time)),
                SANS,
            ),
        };
        let indent = self.commits.indent(i) as f32 * INDENT;
        let theme = cx.global::<Settings>().theme;
        let palette = theme.palette;
        let selected = self.commits.is_selected(i);
        let row = div()
            .id(i)
            .h(s(COMMIT_ROW_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap(s(GAP * 2.))
            .pl(s(PAD_LEFT + indent))
            .pr(s(PAD_RIGHT))
            .whitespace_nowrap();
        selection_and_cursor(row, selected, self.commits.is_cursor(i), focused, &palette)
            .child(self.graph(i, selected, scale_factor, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().font_family(font_family).child(text))
                    .when_some(age, |d, age| {
                        d.child(
                            div()
                                .text_size(s(AGE_SIZE))
                                .text_color(palette.muted)
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
    /// ring for Local changes. The ring is filled with the row's background, so it covers the line.
    ///
    /// GPUI rounds the layout to device pixels, so the line and the dot are whole device pixels
    /// with the lane's parity. Otherwise they are rounded apart and the line is off the dot's center.
    fn graph(&self, i: usize, selected: bool, scale_factor: f32, theme: &UiTheme) -> Div {
        let s = |v: f32| px(v * self.scale);
        let device = |v: f32| (v * self.scale * scale_factor).round().max(1.);
        let lane_width = device(LANE_WIDTH);
        let centered = |v: f32| {
            let width = device(v);
            width + (lane_width - width) % 2.
        };
        let logical = |device: f32| px(device / scale_factor);
        let (line_width, dot_size) = (centered(LINE_WIDTH), centered(DOT_SIZE));
        let lane = div()
            .relative()
            .flex_none()
            .w(logical(lane_width))
            .h_full()
            .flex()
            .items_center()
            .justify_center();
        let row = self.commits.row(i);
        if let CommitRow::Branch(_) = row {
            // Explicitly Embedded, otherwise GPUI reads `branch.svg` as a URI and fetches it over HTTP
            let icon = ImageSource::Resource(Resource::Embedded(branch_icon().into()));
            return lane.child(img(icon).size(s(LANE_WIDTH)));
        }
        let palette = theme.palette;
        let color = theme.commit_graph;
        let line = || {
            div()
                .absolute()
                .left(logical((lane_width - line_width) / 2.))
                .w(logical(line_width))
                .bg(color)
        };
        let up = || line().top(px(-CURSOR_OUTLINE)).bottom(relative(0.5));
        let down = || line().top(relative(0.5)).bottom(px(-CURSOR_OUTLINE));
        let is_commit = matches!(row, CommitRow::Commit(_));
        let dot = div().size(logical(dot_size)).rounded_full().map(|dot| {
            if is_commit {
                dot.bg(color)
            } else {
                dot.border(s(LINE_WIDTH))
                    .border_color(palette.added_pill.text)
                    .bg(if selected {
                        palette.selected
                    } else {
                        palette.surface
                    })
            }
        });
        lane.when(is_commit, |lane| lane.child(up()))
            .when(i + 1 < self.commits.len(), |lane| lane.child(down()))
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
        let scale_factor = window.scale_factor();
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
            .font_family(SANS)
            .text_size(s(TEXT_SIZE))
            .text_color(cx.global::<Settings>().theme.palette.primary)
            .child(
                uniform_list(
                    "commits",
                    self.commits.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| this.render_row(i, now, focused, scale_factor, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .track_scroll(self.scroll.clone()),
            )
    }
}
