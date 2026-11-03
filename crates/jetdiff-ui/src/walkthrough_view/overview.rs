//! The first page: what the diff does, what a reviewer needs to know first, and the notes, with a
//! button at the bottom to the steps. A click on a note goes to its step.

use gpui::{Context, Div, FontWeight, div, prelude::*, px, relative};
use jetdiff_app::walkthrough::{NoteKind, Reading};

use super::notes::NoteRow;
use super::page::{bullet, label, prose_column, wrapping};
use super::palette::{BODY, BUTTON, BUTTON_TEXT, MUTED, STRONG, SUBTITLE, color};
use super::prose::{ProseColors, SANS, prose};
use super::{WalkthroughView, plural};

const TITLE_SIZE: f32 = 32.;
const SUBTITLE_SIZE: f32 = 19.;
const SECTION_SIZE: f32 = 18.;
const TEXT_SIZE: f32 = 16.;
const SMALL_SIZE: f32 = 13.;
const LINE_HEIGHT: f32 = 1.68;
const GAP: f32 = 16.;
const POINT_GAP: f32 = 14.;
const SECTION_GAP: f32 = 40.;
const BUTTON_HEIGHT: f32 = 40.;

impl WalkthroughView {
    pub(super) fn overview(&self, reading: &Reading, cx: &mut Context<Self>) -> Div {
        let s = |v: f32| px(v * self.scale);
        let walkthrough = reading.walkthrough();
        let body = ProseColors {
            text: color(BODY),
            bold: color(STRONG),
        };
        let (title, subtitle) = split_title(&walkthrough.title);
        let steps = reading.step_count();
        let narrow = |content: Div| prose_column(content, self.scale);
        let heading = div()
            .flex()
            .flex_col()
            .child(label(format!("OVERVIEW · {steps} STEPS"), self.scale))
            .child(
                div()
                    .pt(s(GAP / 2.))
                    .text_size(s(TITLE_SIZE))
                    .line_height(relative(1.2))
                    .font_weight(FontWeight::BOLD)
                    .text_color(color(STRONG))
                    .child(title),
            )
            .children(subtitle.map(|subtitle| {
                div()
                    .pt(s(GAP / 4.))
                    .text_size(s(SUBTITLE_SIZE))
                    .line_height(relative(1.4))
                    .text_color(color(SUBTITLE))
                    .child(subtitle)
            }));
        // Prose is centered over the notes, which are a table
        let mut column = div()
            .flex()
            .flex_col()
            .font_family(SANS)
            .text_size(s(TEXT_SIZE))
            .line_height(relative(LINE_HEIGHT))
            .text_color(color(BODY))
            .child(narrow(heading));
        for paragraph in paragraphs(&walkthrough.summary) {
            column = column.child(narrow(
                div()
                    .pt(s(GAP))
                    .child(wrapping(prose(&paragraph, false, &body))),
            ));
        }
        if !walkthrough.before_you_review.is_empty() {
            column = column.child(narrow(section("Before you review", None, self.scale)));
            for point in &walkthrough.before_you_review {
                let point = bullet(wrapping(prose(point, true, &body)), self.scale);
                column = column.child(narrow(div().pt(s(POINT_GAP)).child(point)));
            }
        }
        let notes = reading.notes();
        if !notes.is_empty() {
            let decisions = notes
                .iter()
                .filter(|(_, note)| note.kind == NoteKind::Decision)
                .count();
            let mut count = plural(notes.len(), "note");
            if decisions > 0 {
                count += &format!(" · {}", plural(decisions, "decision"));
            }
            column = column.child(section("Notes", Some(count), self.scale));
            let rows = notes
                .into_iter()
                .map(|(page, note)| NoteRow {
                    note,
                    page: Some(page),
                })
                .collect();
            let table = self.notes_table(rows, cx);
            column = column.child(div().mt(s(POINT_GAP)).child(table));
        }
        column.child(narrow(
            div()
                .pt(s(SECTION_GAP))
                .flex()
                .child(self.continue_button(reading, cx)),
        ))
    }

    /// To the first step not read yet.
    fn continue_button(&self, reading: &Reading, cx: &mut Context<Self>) -> impl IntoElement {
        let s = |v: f32| px(v * self.scale);
        let (page, text) = match reading.first_unread_step() {
            Some(1) if reading.read_count() == 0 => (1, "Start at step 1 →".to_owned()),
            Some(page) => (page, format!("Continue at step {page} →")),
            None => (1, "Back to step 1 →".to_owned()),
        };
        div()
            .id("overview-continue")
            .h(s(BUTTON_HEIGHT))
            .px(s(GAP))
            .flex()
            .items_center()
            .rounded(s(6.))
            .bg(color(BUTTON))
            .text_color(color(BUTTON_TEXT))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .child(text)
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page, cx)))
    }
}

/// A heading with an optional count on the right.
fn section(title: &'static str, count: Option<String>, scale: f32) -> Div {
    div()
        .pt(px(SECTION_GAP * scale))
        .flex()
        .items_baseline()
        .child(
            div()
                .flex_1()
                .text_size(px(SECTION_SIZE * scale))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color(STRONG))
                .child(title),
        )
        .children(count.map(|count| {
            div()
                .text_size(px(SMALL_SIZE * scale))
                .text_color(color(MUTED))
                .child(count)
        }))
}

/// "Walkthrough mode: a change read as steps" is a title and a subtitle.
fn split_title(title: &str) -> (String, Option<String>) {
    let Some((title, subtitle)) = title.split_once(": ") else {
        return (title.to_owned(), None);
    };
    let mut chars = subtitle.chars();
    let mut subtitle: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !subtitle.ends_with(['.', '!', '?']) {
        subtitle.push('.');
    }
    (title.to_owned(), Some(subtitle))
}

/// The last sentence, which is often an aside, in a paragraph of its own.
fn paragraphs(summary: &str) -> Vec<String> {
    let sentence_ends: Vec<usize> = summary.match_indices(". ").map(|(at, _)| at + 1).collect();
    match sentence_ends.last() {
        Some(&end) => vec![
            summary[..end].trim().to_owned(),
            summary[end..].trim().to_owned(),
        ],
        None => vec![summary.trim().to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_and_its_subtitle() {
        assert_eq!(
            split_title("Walkthrough mode: a change read as model-written steps"),
            (
                "Walkthrough mode".into(),
                Some("A change read as model-written steps.".into())
            )
        );
        assert_eq!(split_title("Retry queue"), ("Retry queue".into(), None));
    }

    #[test]
    fn the_last_sentence_in_its_own_paragraph() {
        assert_eq!(
            paragraphs("It opens with w. Then Enter. It is cached."),
            ["It opens with w. Then Enter.", "It is cached."]
        );
        assert_eq!(paragraphs("One sentence."), ["One sentence."]);
    }
}
