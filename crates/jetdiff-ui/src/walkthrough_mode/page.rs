//! A page of the walkthrough: where it is, the step's title, explanation and notes, then its code
//! file by file. After the last step, the files no step covers. The footer has the keys.

use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, Stateful, canvas, div, prelude::*, px, relative,
};
use jetdiff_app::walkthrough::{NoteKind, Page, Reading};
use jetdiff_files::FileStatus;

use super::{Item, State, WalkthroughMode};
use crate::settings::Settings;
use crate::theme::UiTheme;

const PADDING: f32 = 22.;
const GAP: f32 = 12.;
const WHERE_SIZE: f32 = 11.;
const TITLE_SIZE: f32 = 19.;
const TEXT_SIZE: f32 = 14.;
const SMALL_SIZE: f32 = 12.;
/// About 100 characters at the text size. Longer lines of prose are hard to follow.
const TEXT_WIDTH: f32 = 840.;
const CURRENT_BORDER: f32 = 3.;
const FOOTER_HEIGHT: f32 = 36.;

impl WalkthroughMode {
    pub(super) fn ready(&self, cx: &mut Context<Self>) -> Div {
        let State::Ready(reading) = &self.state else {
            return div();
        };
        let theme = cx.global::<Settings>().theme;
        // A scrolled column shrinks its children to fit unless told not to, and text then overlaps
        let children: Vec<AnyElement> = (0..self.items.len())
            .map(|item| {
                let element = self.item(item, reading, &theme, cx);
                div().flex_none().w_full().child(element).into_any_element()
            })
            .collect();
        let s = |v: f32| px(v * self.scale);
        let page = div()
            .id("walkthrough-page")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px(s(PADDING))
            .py(s(PADDING * 0.75))
            .flex()
            .flex_col()
            .gap(s(GAP))
            .children(children);
        let contents = self.contents(reading, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().flex().child(contents).child(page))
            .child(self.footer(reading, cx))
    }

    fn item(
        &self,
        item: usize,
        reading: &Reading,
        theme: &UiTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = |v: f32| px(v * self.scale);
        let current = self.current == Some(item);
        match &self.items[item] {
            // A flex item gets the page's width up to its max, so the text wraps with the window
            Item::Heading => {
                let heading = match reading.page() {
                    Page::Overview => self.overview(reading, theme, cx),
                    Page::Step { .. } | Page::NotCovered => self.heading(reading, theme),
                };
                div()
                    .flex()
                    .child(heading.flex_1().min_w_0())
                    .into_any_element()
            }
            Item::File(file) => {
                let file = &self.digest.as_ref().expect("a page has a digest").files()[*file].file;
                let status = match file.status {
                    FileStatus::Added => Some("added".to_owned()),
                    FileStatus::Deleted => Some("deleted".to_owned()),
                    FileStatus::Modified => None,
                    FileStatus::Renamed => file.renamed_from_label(),
                };
                div()
                    .pt(s(GAP))
                    .flex()
                    .gap(s(GAP))
                    .text_size(s(SMALL_SIZE + 1.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(file.path.clone()),
                    )
                    .children(
                        status.map(|status| div().text_color(theme.line_number).child(status)),
                    )
                    .into_any_element()
            }
            Item::Excerpt { excerpt, .. } => {
                let Some(style) = self.style.clone() else {
                    return div().into_any_element();
                };
                let height = excerpt.row_count() as f32 * style.line_height();
                let (excerpt, theme) = (excerpt.clone(), *theme);
                let border = if current {
                    theme.file_modified
                } else {
                    theme.indent_guide
                };
                div()
                    .id(("excerpt", item))
                    .flex_none()
                    .border_l(px(CURRENT_BORDER))
                    .border_color(border)
                    .rounded(s(4.))
                    .overflow_hidden()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, cx| {
                                excerpt.paint(style.clone(), theme, bounds, window, cx)
                            },
                        )
                        .w_full()
                        .h(px(height)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.current = Some(item);
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Item::NotCovered(index) => {
                let not_covered = &reading.not_covered()[*index];
                let path = self.digest.as_ref().expect("a page has a digest").files()
                    [not_covered.file]
                    .file
                    .path
                    .clone();
                let hunks = match not_covered.hunks.len() {
                    0 => String::new(),
                    1 => "1 hunk".into(),
                    n => format!("{n} hunks"),
                };
                div()
                    .id(("not-covered", item))
                    .px(s(GAP))
                    .py(s(GAP / 2.))
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_l(px(CURRENT_BORDER))
                    .border_color(if current {
                        theme.file_modified
                    } else {
                        theme.indent_guide
                    })
                    .rounded(s(6.))
                    .cursor_pointer()
                    .text_size(s(SMALL_SIZE + 1.))
                    .child(
                        div()
                            .flex()
                            .gap(s(GAP))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(path),
                            )
                            .child(div().text_color(theme.line_number).child(hunks)),
                    )
                    .when(!not_covered.note.is_empty(), |d| {
                        d.child(wrapping(not_covered.note.clone()).text_color(theme.line_number))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.current = Some(item);
                        this.open_current(cx);
                    }))
                    .into_any_element()
            }
        }
    }

    /// Where the page is, the title with Key change, the explanation and the notes.
    fn heading(&self, reading: &Reading, theme: &UiTheme) -> Div {
        let s = |v: f32| px(v * self.scale);
        let title = |text: String| {
            div()
                .text_size(s(TITLE_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        let where_ = |text: String| {
            div()
                .text_size(s(WHERE_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.line_number)
                .child(text)
        };
        let column = div()
            .max_w(s(TEXT_WIDTH))
            .flex()
            .flex_col()
            .gap(s(GAP))
            .text_size(s(TEXT_SIZE))
            .line_height(relative(1.5));
        let Some((step, number, steps)) = reading.step() else {
            return column
                .child(where_("NOT COVERED".into()))
                .child(title("Not covered".into()))
                .child(
                    wrapping(
                        "The hunks no step covers, and the files that weren't sent or read. \
                         Enter opens one in the diff."
                            .into(),
                    )
                    .text_color(theme.line_number),
                );
        };
        let part = reading
            .part()
            .map_or(String::new(), |part| part.title.to_uppercase());
        let notes = step.notes.iter().map(|note| {
            let kind = note_kind(note.kind);
            div()
                .flex()
                .items_start()
                .gap(s(GAP))
                .text_size(s(SMALL_SIZE + 1.))
                .child(chip(
                    kind,
                    theme.notification_warning_background,
                    theme.notification_foreground,
                    self.scale,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(theme.line_number)
                        .child(note.text.clone()),
                )
        });
        let paragraphs = step
            .text
            .split("\n\n")
            .filter(|paragraph| !paragraph.trim().is_empty())
            .map(|paragraph| wrapping(paragraph.trim().to_owned()));
        column
            .child(where_(format!("{part} · STEP {number} OF {steps}")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(GAP))
                    .child(title(step.title.clone()))
                    .when(step.is_key(), |d| {
                        d.child(chip(
                            "Key change",
                            theme.tree_selection,
                            theme.editor_fg,
                            self.scale,
                        ))
                    }),
            )
            .children(paragraphs)
            .children(notes)
    }

    fn footer(&self, reading: &Reading, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let theme = cx.global::<Settings>().theme;
        let button = |id: &'static str,
                      text: &'static str,
                      forward: bool,
                      cx: &mut Context<Self>|
         -> Stateful<Div> {
            div()
                .id(id)
                .px(s(GAP))
                .py(s(3.))
                .border_1()
                .border_color(theme.indent_guide)
                .rounded(s(5.))
                .cursor_pointer()
                .child(text)
                .on_click(cx.listener(move |this, _, _, cx| this.step_by(forward, cx)))
        };
        let before_not_covered = reading
            .step()
            .is_some_and(|(_, number, steps)| number == steps)
            && !reading.not_covered().is_empty();
        let next = if before_not_covered {
            "Not covered →"
        } else {
            "Next →"
        };
        let read = if reading.is_last() {
            "End of the walkthrough".to_owned()
        } else {
            format!("{} of {} read", reading.seen_count(), reading.step_count())
        };
        div()
            .flex_none()
            .h(s(FOOTER_HEIGHT))
            .px(s(GAP))
            .flex()
            .items_center()
            .gap(s(GAP))
            .border_t_1()
            .border_color(theme.indent_guide)
            .text_size(s(SMALL_SIZE))
            .child(button("previous", "← Previous", false, cx))
            .child(button("next", next, true, cx))
            .child(div().text_color(theme.line_number).child(read))
            .child(div().flex_1())
            .child(
                div()
                    .text_color(theme.line_number)
                    .child("Tab → step   n p hunk   Enter open in the diff   w back"),
            )
    }
}

/// Text that wraps at the width of the page. In the page's scroll, only a flex item that may shrink
/// gets a width to wrap at; elsewhere the text is measured as one line, and runs off or overlaps.
pub(super) fn wrapping(text: String) -> Div {
    div().flex().child(div().flex_1().min_w_0().child(text))
}

/// What the note is called on the page.
pub(super) fn note_kind(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Choice => "Decision",
        NoteKind::Upkeep => "Upkeep",
        NoteKind::Gap => "Limitation",
        NoteKind::Divergence => "Divergence",
        NoteKind::Untested => "Untested",
    }
}

pub(super) fn chip(text: &'static str, background: Hsla, foreground: Hsla, scale: f32) -> Div {
    div()
        .flex_none()
        .px(px(6. * scale))
        .rounded(px(4. * scale))
        .bg(background)
        .text_color(foreground)
        .text_size(px(SMALL_SIZE * scale))
        .child(text)
}
