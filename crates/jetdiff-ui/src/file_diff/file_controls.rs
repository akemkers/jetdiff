//! The file controls at the right of the diff header: the diff mode and Fold. A button that can't
//! do anything is dimmed, not hidden, so the row keeps its layout.

use gpui::{
    App, ClickEvent, Context, Div, FontWeight, SharedString, Stateful, Window, div, prelude::*, px,
};
use jetdiff_viewmodel::{Command, DiffMode};

use super::{FileDiffView, ToggleDiffMode};
use crate::text::SANS;
use crate::theme::UiPalette;

const HEIGHT: f32 = 24.;
const PADDING: f32 = 10.;
const RADIUS: f32 = 5.;
const TEXT_SIZE: f32 = 12.;
const DIMMED_OPACITY: f32 = 0.4;
const GAP: f32 = 10.;

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

struct Button {
    label: SharedString,
    active: bool,
    enabled: bool,
    on_click: OnClick,
}

impl Button {
    fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            active: false,
            enabled: true,
            on_click: Box::new(on_click),
        }
    }

    fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl FileDiffView {
    pub(super) fn file_controls(
        &self,
        palette: &UiPalette,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GAP * scale))
            .child(self.diff_mode(palette, scale, cx))
            .child(self.fold(palette, scale, cx))
    }

    /// A click on the side not shown does what `v` does.
    fn diff_mode(&self, palette: &UiPalette, scale: f32, cx: &mut Context<Self>) -> Div {
        let split = matches!(self.mode, DiffMode::Split(_));
        let buttons = vec![
            Button::new("Split", toggle_diff_mode(cx))
                .active(split)
                .enabled(self.mode.diff().can_split()),
            Button::new("Unified", toggle_diff_mode(cx)).active(!split),
        ];
        group("diff-mode", buttons, palette, scale)
    }

    /// Does what `f` does, which only works in Split until folds come to Unified.
    fn fold(&self, palette: &UiPalette, scale: f32, cx: &mut Context<Self>) -> Div {
        let split = matches!(self.mode, DiffMode::Split(_));
        let fold = Button::new(
            "Fold",
            cx.listener(|this, _, window, cx| this.execute(Command::ToggleFolds, window, cx)),
        )
        .active(split && self.fold_unchanged)
        .enabled(split);
        toggle("fold", fold, palette, scale)
    }
}

fn toggle_diff_mode(
    cx: &mut Context<FileDiffView>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(|this, _, window, cx| this.toggle_diff_mode(&ToggleDiffMode, window, cx))
}

/// Buttons side by side where the one that is on is filled, and a click on another picks it.
fn group(id: &'static str, buttons: Vec<Button>, palette: &UiPalette, scale: f32) -> Div {
    frame(palette, scale).children(
        buttons
            .into_iter()
            .enumerate()
            .map(|(i, button)| button_element((id, i), button, false, palette, scale)),
    )
}

/// One button that is filled while it is on, and a click turns it on or off.
fn toggle(id: &'static str, button: Button, palette: &UiPalette, scale: f32) -> Div {
    frame(palette, scale).child(button_element((id, 0), button, true, palette, scale))
}

fn frame(palette: &UiPalette, scale: f32) -> Div {
    div()
        .flex_none()
        .flex()
        .border_1()
        .border_color(palette.border)
        .rounded(px(RADIUS * scale))
        .overflow_hidden()
        .font_family(SANS)
        .text_size(px(TEXT_SIZE * scale))
}

fn button_element(
    id: (&'static str, usize),
    button: Button,
    clickable_when_active: bool,
    palette: &UiPalette,
    scale: f32,
) -> Stateful<Div> {
    let Button {
        label,
        active,
        enabled,
        on_click,
    } = button;
    let palette = *palette;
    div()
        .id(id)
        .h(px(HEIGHT * scale))
        .px(px(PADDING * scale))
        .flex()
        .items_center()
        .when(id.1 > 0, |d| d.border_l_1().border_color(palette.border))
        .map(|d| match active {
            true => d
                .bg(palette.selected)
                .text_color(palette.strong)
                .font_weight(FontWeight::SEMIBOLD),
            false => d.text_color(palette.muted),
        })
        .map(|d| match (enabled, active && !clickable_when_active) {
            (false, _) => d.opacity(DIMMED_OPACITY),
            (true, true) => d,
            (true, false) => d
                .cursor_pointer()
                .when(!active, |d| d.hover(|d| d.text_color(palette.secondary)))
                .on_click(on_click),
        })
        .child(label)
}
