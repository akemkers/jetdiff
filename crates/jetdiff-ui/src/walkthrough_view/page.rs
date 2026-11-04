//! A page of the walkthrough: where it is, the step's title, explanation and notes, then its code
//! file by file. After the last step, the files no step covers. Under it, the keymap bar.

use gpui::{AnyElement, Context, Div, FontWeight, Stateful, canvas, div, prelude::*, px, relative};
use jetdiff_app::walkthrough::{Page, Reading};
use jetdiff_files::FileStatus;

use super::notes::NoteRow;
use super::palette::{ACCENT, BODY, BORDER, FILE_HEADER, MUTED, PAGE, SELECTED, STRONG, color};
use super::prose::{ProseColors, SANS, prose};
use super::{Item, State, WalkthroughView};
use crate::settings::Settings;
use crate::text::FAMILY;
use crate::theme::UiTheme;

const TOP_PADDING: f32 = 40.;
const SIDE_PADDING: f32 = 48.;
const GAP: f32 = 12.;
/// Space above a file, so it stands apart from the code of the file before.
const FILE_GAP: f32 = 20.;
const FILE_HEADER_PADDING: f32 = 8.;
const LABEL_SIZE: f32 = 12.;
const TITLE_SIZE: f32 = 24.;
const TEXT_SIZE: f32 = 16.;
const SMALL_SIZE: f32 = 13.;
const LINE_HEIGHT: f32 = 1.65;
/// About 68 characters at the text size, since long lines are hard to follow.
const PROSE_WIDTH: f32 = 600.;
const TAG_HEIGHT: f32 = 22.;
const DOT_SIZE: f32 = 5.;
const POINT_GAP: f32 = 14.;
const CURRENT_BORDER: f32 = 3.;
const KEYMAP_BAR_HEIGHT: f32 = 44.;

impl WalkthroughView {
    pub(super) fn ready(&self, cx: &mut Context<Self>) -> Div {
        let State::Ready(reading) = &self.state else {
            return div();
        };
        let theme = cx.global::<Settings>().theme;
        // A scrolled column shrinks its children to fit unless told not to, and text then overlaps.
        // Each item grows to the column's width.
        let children: Vec<AnyElement> = (0..self.items.len())
            .map(|item| {
                let element = self.item(item, reading, &theme, cx);
                let column = div().flex_1().min_w_0().child(element);
                div()
                    .flex_none()
                    .w_full()
                    .flex()
                    .child(column)
                    .into_any_element()
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
            .px(s(SIDE_PADDING))
            .pt(s(TOP_PADDING))
            .pb(s(TOP_PADDING))
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
            .child(self.keymap_bar(reading, cx))
    }

    fn item(
        &self,
        item: usize,
        reading: &Reading,
        theme: &UiTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = |v: f32| px(v * self.scale);
        let is_cursor = self.cursor == Some(item);
        match &self.items[item] {
            // A flex item gets the page's width up to its max, so the text wraps with the window
            Item::Heading => {
                let heading = match reading.page() {
                    Page::Overview => self.overview(reading, cx),
                    Page::Step { .. } | Page::NotCovered => self.heading(reading, cx),
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
                // The folder gives way when the path is long, and the file name always shows
                let (folder, name) = match file.path.rsplit_once('/') {
                    Some((folder, name)) => (Some(format!("{folder}/")), name.to_owned()),
                    None => (None, file.path.clone()),
                };
                div()
                    .id(("file", item))
                    .mt(s(FILE_GAP))
                    .px(s(GAP))
                    .py(s(FILE_HEADER_PADDING))
                    .flex()
                    .gap(s(GAP))
                    .bg(color(FILE_HEADER))
                    .border_1()
                    .border_color(color(BORDER))
                    .rounded(s(6.))
                    .cursor_pointer()
                    .hover(|d| d.bg(color(SELECTED)).border_color(color(ACCENT)))
                    .text_size(s(SMALL_SIZE))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .font_family(FAMILY)
                            .children(folder.map(|folder| {
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(color(MUTED))
                                    .child(folder)
                            }))
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(STRONG))
                                    .child(name),
                            ),
                    )
                    .children(
                        status
                            .map(|status| div().flex_none().text_color(color(MUTED)).child(status)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.open_file(item, cx)))
                    .into_any_element()
            }
            Item::Excerpt { excerpt, .. } => {
                let Some(style) = self.style.clone() else {
                    return div().into_any_element();
                };
                let height = excerpt.line_count() as f32 * style.line_height();
                let (excerpt, theme) = (excerpt.clone(), *theme);
                let border = if is_cursor {
                    color(ACCENT)
                } else {
                    color(BORDER)
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
                        this.cursor = Some(item);
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Item::NotCovered { index, too_long } => {
                let not_covered = &reading.not_covered()[*index];
                let path = self.digest.as_ref().expect("a page has a digest").files()
                    [not_covered.file]
                    .file
                    .path
                    .clone();
                let mut hunks = match not_covered.hunks.len() {
                    0 => String::new(),
                    1 => "1 hunk".into(),
                    n => format!("{n} hunks"),
                };
                if *too_long {
                    hunks += ", too long to show here";
                }
                let muted = ProseColors {
                    text: color(MUTED),
                    bold: color(BODY),
                };
                div()
                    .id(("not-covered", item))
                    .px(s(GAP * 1.5))
                    .py(s(GAP))
                    .flex()
                    .flex_col()
                    .gap(s(GAP / 3.))
                    .border_1()
                    .border_l(px(CURRENT_BORDER))
                    .border_color(if is_cursor {
                        color(ACCENT)
                    } else {
                        color(BORDER)
                    })
                    .rounded(s(8.))
                    .cursor_pointer()
                    .text_size(s(SMALL_SIZE))
                    .child(
                        div()
                            .flex()
                            .gap(s(GAP))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(FAMILY)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(STRONG))
                                    .child(path),
                            )
                            .child(div().text_color(color(MUTED)).child(hunks)),
                    )
                    .when(!not_covered.note.is_empty(), |d| {
                        d.child(wrapping(prose(&not_covered.note, false, &muted)))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.cursor = Some(item);
                        this.open_at_cursor(cx);
                    }))
                    .into_any_element()
            }
        }
    }

    /// Where the page is, the title with Key step, and the points, at the left edge of the code.
    /// The notes under them are a table as wide as the text.
    fn heading(&self, reading: &Reading, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let body = ProseColors {
            text: color(BODY),
            bold: color(STRONG),
        };
        let title = |text: String| {
            div()
                .text_size(s(TITLE_SIZE))
                .line_height(relative(1.3))
                .font_weight(FontWeight::BOLD)
                .text_color(color(STRONG))
                .child(text)
        };
        let column = div()
            .flex()
            .flex_col()
            .gap(s(GAP * 1.25))
            .font_family(SANS)
            .text_size(s(TEXT_SIZE))
            .line_height(relative(LINE_HEIGHT))
            .text_color(color(BODY));
        let Some((step, number, steps)) = reading.step() else {
            let not_covered = column
                .child(label("NOT COVERED".into(), self.scale))
                .child(title("Not covered".into()))
                .child(wrapping(prose(
                    "The hunks no step covers, and the files that weren't sent or read. `Enter` \
                     opens one in the diff.",
                    false,
                    &body,
                )));
            return prose_column(not_covered, self.scale);
        };
        let part = reading
            .part()
            .map_or(String::new(), |part| part.title.to_uppercase());
        let points = step
            .points
            .iter()
            .map(|point| bullet(wrapping(prose(point, false, &body)), self.scale));
        let top = column
            .child(label(
                format!("{part} · STEP {number} OF {steps}"),
                self.scale,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(GAP))
                    .child(title(step.title.clone()))
                    .when(step.is_key(), |d| d.child(key_step(self.scale))),
            )
            .children(points);
        let notes = step
            .notes
            .iter()
            .map(|note| NoteRow { note, page: None })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap(s(GAP * 1.5))
            .child(prose_column(top, self.scale))
            .when(!notes.is_empty(), |d| {
                d.child(prose_column(self.notes_table(notes, cx), self.scale))
            })
    }

    /// The keys, with buttons for the previous and next page, and how much is read.
    fn keymap_bar(&self, reading: &Reading, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let button = |id: &'static str,
                      text: &'static str,
                      forward: bool,
                      cx: &mut Context<Self>|
         -> Stateful<Div> {
            div()
                .id(id)
                .h(s(30.))
                .px(s(GAP))
                .flex()
                .items_center()
                .border_1()
                .border_color(color(BORDER))
                .rounded(s(6.))
                .cursor_pointer()
                .hover(|d| d.bg(color(SELECTED)))
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
            format!("{} of {} read", reading.read_count(), reading.step_count())
        };
        div()
            .flex_none()
            .h(s(KEYMAP_BAR_HEIGHT))
            .px(s(GAP * 1.5))
            .flex()
            .items_center()
            .gap(s(GAP))
            .bg(color(PAGE))
            .border_t_1()
            .border_color(color(BORDER))
            .font_family(SANS)
            .text_size(s(SMALL_SIZE))
            .child(button("previous", "← Previous", false, cx))
            .child(button("next", next, true, cx))
            .child(div().text_color(color(MUTED)).child(read))
            .child(div().flex_1())
            .child(
                div().font_family(FAMILY).text_color(color(MUTED)).child(
                    "Tab → step   n p hunk   v split/unified   Enter open in the diff   w back",
                ),
            )
    }
}

/// Prose and notes in a column narrower than the code, at its left edge.
pub(super) fn prose_column(content: impl IntoElement, scale: f32) -> Div {
    let column = div()
        .flex_1()
        .min_w_0()
        .max_w(px(PROSE_WIDTH * scale))
        .child(content);
    div().flex().child(column)
}

/// A point with a small dot in front, as in a list.
pub(super) fn bullet(content: impl IntoElement, scale: f32) -> Div {
    let s = |v: f32| px(v * scale);
    let dot = div()
        .flex_none()
        .mt(s(TEXT_SIZE * LINE_HEIGHT / 2. - DOT_SIZE / 2.))
        .size(s(DOT_SIZE))
        .rounded_full()
        .bg(color(MUTED));
    div()
        .flex()
        .items_start()
        .gap(s(POINT_GAP))
        .child(dot)
        .child(div().flex_1().min_w_0().child(content))
}

/// Wraps at the width of the page. In the page's scroll, only a flex item that may shrink gets a
/// width to wrap at; elsewhere text is measured as one line, and runs off or overlaps.
pub(super) fn wrapping(text: impl IntoElement) -> Div {
    div().flex().child(div().flex_1().min_w_0().child(text))
}

/// A small heading above the content, like `OVERVIEW · 16 STEPS`.
pub(super) fn label(text: String, scale: f32) -> Div {
    div()
        .text_size(px(LABEL_SIZE * scale))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(MUTED))
        .child(text)
}

pub(super) fn key_step(scale: f32) -> Div {
    div()
        .flex_none()
        .h(px(TAG_HEIGHT * scale))
        .px(px(10. * scale))
        .flex()
        .items_center()
        .rounded_full()
        .bg(color(SELECTED))
        .text_color(color(ACCENT))
        .text_size(px(LABEL_SIZE * scale))
        .line_height(relative(1.))
        .font_weight(FontWeight::SEMIBOLD)
        .child("Key step")
}
