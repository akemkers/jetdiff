//! Before a walkthrough is shown: what will be read, then the progress while the model writes, or why
//! it couldn't.

use gpui::{Context, Div, FontWeight, Hsla, div, prelude::*, px};
use jetdiff_app::walkthrough::FileContent;
use jetdiff_files::FileStatus;
use jetdiff_viewmodel::Side;

use super::{State, WalkthroughMode};
use crate::settings::Settings;

const COLUMN_WIDTH: f32 = 640.;
const HEADING_SIZE: f32 = 18.;
const TEXT_SIZE: f32 = 13.;
const SMALL_SIZE: f32 = 12.;
const GAP: f32 = 14.;
const FILE_ROW_HEIGHT: f32 = 24.;

impl WalkthroughMode {
    pub(super) fn intro(&self, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let theme = cx.global::<Settings>().theme;
        let settings = cx.global::<Settings>();
        let (model, language) = (&settings.walkthrough.model, &settings.walkthrough.language);
        let column = div()
            .w(s(COLUMN_WIDTH))
            .flex()
            .flex_col()
            .gap(s(GAP))
            .text_size(s(TEXT_SIZE));
        let dim = |text: String| div().text_color(theme.line_number).child(text);
        let body = match &self.state {
            State::Loading => column.child(dim("Reading the change…".into())),
            State::Writing { started, .. } => {
                let seconds = started.elapsed().as_secs();
                column
                    .child(self.intro_heading(format!("Asking Claude {model}…"), cx))
                    .child(dim(format!("{}:{:02}", seconds / 60, seconds % 60)))
                    .child(dim(
                        "It goes on in the background if you go back to the diff with w.".into(),
                    ))
                    .child(
                        div()
                            .id("cancel")
                            .cursor_pointer()
                            .text_color(theme.file_modified)
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
            }
            State::Failed(error) => column
                .child(self.intro_heading("The walkthrough couldn't be written".into(), cx))
                .child(div().text_color(theme.file_deleted).child(error.clone()))
                .child(dim("Enter tries again. w goes back to the diff.".into())),
            State::Intro | State::Ready(_) => {
                let mut column = column
                    .child(
                        self.intro_heading(
                            "Read this change in the order it makes sense".into(),
                            cx,
                        ),
                    )
                    .child(dim(format!(
                        "Claude {model} groups the hunks into steps, puts them in an order where \
                         each builds on the last, and explains each one. Written in {language}."
                    )))
                    .child(self.counts(cx));
                let digest = self.digest.as_ref();
                let action = match digest {
                    Some(digest) if digest.hunk_count() == 0 => {
                        dim("Nothing to walk through.".into())
                    }
                    Some(digest) if digest.is_too_large() => div()
                        .text_color(theme.file_deleted)
                        .child("Too large for one walkthrough. Read a part of it, like a commit."),
                    _ => div().child(
                        div()
                            .text_color(theme.file_modified)
                            .child("Enter writes it"),
                    ),
                };
                column = column.child(action).child(self.file_list(cx));
                column
            }
        };
        div()
            .size_full()
            .flex()
            .justify_center()
            .pt(s(GAP * 4.))
            .child(body)
    }

    fn intro_heading(&self, text: String, cx: &Context<Self>) -> Div {
        div()
            .text_size(px(HEADING_SIZE * self.scale))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.global::<Settings>().theme.editor_fg)
            .child(text)
    }

    /// Files and hunks, and what isn't sent or read.
    fn counts(&self, cx: &Context<Self>) -> Div {
        let theme = cx.global::<Settings>().theme;
        let Some(digest) = &self.digest else {
            return div();
        };
        let files = digest.files();
        let generated = files
            .iter()
            .filter(|file| matches!(file.content, FileContent::Generated))
            .count();
        let not_read = files
            .iter()
            .filter(|file| matches!(file.content, FileContent::NotRead(_)))
            .count();
        let mut counts = vec![
            format!("{} files", files.len()),
            format!("{} hunks", digest.hunk_count()),
        ];
        if generated > 0 {
            counts.push(format!("{generated} generated, not sent"));
        }
        if not_read > 0 {
            counts.push(format!("{not_read} not read"));
        }
        div()
            .flex()
            .gap(px(GAP * self.scale))
            .text_size(px(SMALL_SIZE * self.scale))
            .text_color(theme.line_number)
            .children(counts)
    }

    fn file_list(&self, cx: &Context<Self>) -> Div {
        let theme = cx.global::<Settings>().theme;
        let s = |v: f32| px(v * self.scale);
        let Some(digest) = &self.digest else {
            return div();
        };
        let rows = digest.files().iter().map(|file| {
            let status = match file.file.status {
                FileStatus::Added => theme.file_added,
                FileStatus::Deleted => theme.file_deleted,
                FileStatus::Modified | FileStatus::Renamed => theme.file_modified,
            };
            let lines = match &file.content {
                FileContent::Diff(diff) => {
                    let count = |side: Side| -> usize {
                        diff.hunks().iter().map(|hunk| hunk.lines(side).len()).sum()
                    };
                    Some((count(Side::Right), count(Side::Left)))
                }
                FileContent::Generated => None,
                FileContent::NotRead(_) => None,
            };
            let note = match &file.content {
                FileContent::Generated => Some("generated".to_owned()),
                FileContent::NotRead(reason) => Some(reason.clone()),
                FileContent::Diff(_) => None,
            };
            div()
                .h(s(FILE_ROW_HEIGHT))
                .px(s(GAP / 2.))
                .flex()
                .items_center()
                .gap(s(GAP / 2.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(status)
                        .child(file.file.path.clone()),
                )
                .children(note.map(|note| line_number_text(note, theme.line_number)))
                .children(lines.map(|(added, deleted)| {
                    div()
                        .flex()
                        .gap(s(GAP / 2.))
                        .child(
                            div()
                                .text_color(theme.file_added)
                                .child(format!("+{added}")),
                        )
                        .child(
                            div()
                                .text_color(theme.file_deleted)
                                .child(format!("−{deleted}")),
                        )
                }))
        });
        div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(theme.indent_guide)
            .rounded(s(6.))
            .text_size(s(SMALL_SIZE))
            .children(rows)
    }
}

fn line_number_text(text: String, color: Hsla) -> Div {
    div().flex_none().text_color(color).child(text)
}
