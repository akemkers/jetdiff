//! The side panel, the same next to the diff and in the walkthrough: the walkthrough switch at the
//! top, then the file tree with the commit list under it, or the walkthrough's contents. The width
//! follows the file tree in both, so `w` only changes the content.

use gpui::{
    Context, Div, Focusable, FontWeight, ImageSource, Resource, Stateful, Window, div, img,
    prelude::*, px,
};

use super::RepoView;
use crate::button_group::{Button, group};
use crate::icons::chevron_icon;
use crate::list::{PAD_LEFT, ROW_HEIGHT, TEXT_SIZE};
use crate::settings::Settings;
use crate::side_panel_size::{SectionHeight, SidePanelWidth};
use crate::text::{FAMILY, SANS};

/// As in the walkthrough's contents.
const PADDING: f32 = 7.;
const LABEL_SIZE: f32 = 11.;
const CHEVRON_SIZE: f32 = 14.;
const HEADING_GAP: f32 = 8.;

impl RepoView {
    pub(super) fn side_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let content = match self.walkthrough_view() {
            Some(view) => view
                .update(cx, |view, cx| view.contents(cx))
                .unwrap_or_else(div),
            None => self.sections(window, cx),
        };
        let content_width = self.files.update(cx, |tree, _| tree.content_width(window));
        let window_width = f32::from(window.viewport_size().width) / self.scale;
        let width = self.width.resolve(content_width, window_width);
        let palette = cx.global::<Settings>().theme.palette;
        let side_panel = div()
            .flex_none()
            .w(px(width * self.scale))
            .h_full()
            .flex()
            .flex_col()
            .bg(palette.surface)
            .border_r_1()
            .border_color(palette.border)
            .child(self.walkthrough_switch(cx))
            .child(div().flex_1().min_h_0().child(content));
        SidePanelWidth::attach(side_panel, cx, |this| (&mut this.width, this.scale))
    }

    /// A click on the side not shown does what `w` does, so `w` is shown there.
    fn walkthrough_switch(&self, cx: &mut Context<Self>) -> Div {
        let shown = self.walkthrough_view().is_some();
        let button = |label: &'static str, active: bool| {
            Button::new(
                label,
                cx.listener(|this, _, window, cx| this.toggle_walkthrough(window, cx)),
            )
            .active(active)
            .key((!active).then_some("w"))
        };
        let buttons = vec![button("Diff", !shown), button("Walkthrough", shown)];
        let palette = cx.global::<Settings>().theme.palette;
        div()
            .flex_none()
            .p(px(PADDING * self.scale))
            .flex()
            .child(group("walkthrough-switch", buttons, &palette, self.scale))
    }

    /// The file tree with the commit list under it, above a divider. A closed section moves to the
    /// bottom, and without the file tree the commit list takes the rest.
    fn sections(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let scaled = |size: f32| px(size * self.scale);
        let file_count = self.repo_source.changed_files().len();
        let border = cx.global::<Settings>().theme.palette.border;
        let sections = div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(FAMILY)
            .text_size(scaled(TEXT_SIZE))
            .child(
                self.heading("FILES", Some(file_count), self.files_open, cx, |this| {
                    &mut this.files_open
                }),
            )
            .when(self.files_open, |section| {
                section.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px(scaled(PADDING))
                        .child(self.files.clone()),
                )
            });
        let Some(commits) = self.commits.clone() else {
            return sections;
        };
        let section = div()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(border)
            .child(
                self.heading("COMMITS", None, self.commits_open, cx, |this| {
                    &mut this.commits_open
                }),
            )
            .when(self.commits_open, |section| {
                section.child(div().flex_1().min_h_0().px(scaled(PADDING)).child(commits))
            });
        match (self.files_open, self.commits_open) {
            (true, true) => {
                let window_height = f32::from(window.viewport_size().height) / self.scale;
                let height = self.commits_height.resolve(window_height);
                SectionHeight::attach(sections, section.h(scaled(height)), cx, |this| {
                    (&mut this.commits_height, this.scale)
                })
            }
            (false, true) => sections.child(section.flex_1().min_h_0()),
            (true, false) => sections.child(section),
            (false, false) => sections.child(div().flex_1()).child(section),
        }
    }

    /// The section's name, with a chevron at the right that points down when it is open. A click
    /// opens or closes the section.
    fn heading(
        &self,
        title: &'static str,
        count: Option<usize>,
        open: bool,
        cx: &mut Context<Self>,
        open_flag: fn(&mut Self) -> &mut bool,
    ) -> Stateful<Div> {
        let scaled = |size: f32| px(size * self.scale);
        // Explicitly Embedded, otherwise GPUI reads the file name as a URI and fetches it over HTTP
        let chevron = ImageSource::Resource(Resource::Embedded(chevron_icon(open).into()));
        div()
            .id(title)
            .flex_none()
            .h(scaled(ROW_HEIGHT))
            .px(scaled(PADDING + PAD_LEFT))
            .flex()
            .items_center()
            .gap(scaled(HEADING_GAP))
            .font_family(SANS)
            .text_size(scaled(LABEL_SIZE))
            .text_color(cx.global::<Settings>().theme.palette.muted)
            .cursor_pointer()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .children(count.map(|count| count.to_string()))
            .child(div().flex_1())
            .child(img(chevron).flex_none().size(scaled(CHEVRON_SIZE)))
            .on_click(cx.listener(move |this, _, window, cx| {
                let open = open_flag(this);
                *open = !*open;
                this.closed_section_loses_focus(window, cx);
                cx.notify();
            }))
    }

    fn closed_section_loses_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let tree = self.files.focus_handle(cx);
        let commits = self.commits.as_ref().map(|list| list.focus_handle(cx));
        let hidden = (!self.files_open && tree.is_focused(window))
            || (!self.commits_open && commits.is_some_and(|list| list.is_focused(window)));
        if hidden {
            window.focus(&self.diff.focus_handle(cx));
        }
    }
}
