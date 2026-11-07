//! Buttons in a frame, side by side: the file controls and the walkthrough switch. A button that
//! can't do anything is dimmed, not hidden, so the row keeps its layout.

use gpui::{App, ClickEvent, Div, FontWeight, SharedString, Stateful, Window, div, prelude::*, px};

use crate::help::key_label;
use crate::text::SANS;
use crate::theme::UiPalette;

const HEIGHT: f32 = 30.;
const PADDING: f32 = 12.;
const RADIUS: f32 = 5.;
const TEXT_SIZE: f32 = 14.;
const DIMMED_OPACITY: f32 = 0.4;
const KEY_LABEL_GAP: f32 = 6.;

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub struct Button {
    label: SharedString,
    active: bool,
    enabled: bool,
    /// Shown after the label, for the key that does the same.
    key: Option<&'static str>,
    on_click: OnClick,
}

impl Button {
    pub fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            active: false,
            enabled: true,
            key: None,
            on_click: Box::new(on_click),
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn key(mut self, key: Option<&'static str>) -> Self {
        self.key = key;
        self
    }
}

/// Buttons side by side where the one that is on is filled, and a click on another picks it.
pub fn group(id: &'static str, buttons: Vec<Button>, palette: &UiPalette, scale: f32) -> Div {
    frame(palette, scale).children(
        buttons
            .into_iter()
            .enumerate()
            .map(|(i, button)| button_element((id, i), button, false, palette, scale)),
    )
}

/// One button that is filled while it is on, and a click turns it on or off.
pub fn toggle(id: &'static str, button: Button, palette: &UiPalette, scale: f32) -> Div {
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
        key,
        on_click,
    } = button;
    let palette = *palette;
    div()
        .id(id)
        .h(px(HEIGHT * scale))
        .px(px(PADDING * scale))
        .flex()
        .items_center()
        .gap(px(KEY_LABEL_GAP * scale))
        .when(id.1 > 0, |element| {
            element.border_l_1().border_color(palette.border)
        })
        .map(|element| match active {
            true => element
                .bg(palette.selected)
                .text_color(palette.strong)
                .font_weight(FontWeight::SEMIBOLD),
            false => element.text_color(palette.muted),
        })
        .map(
            |element| match (enabled, active && !clickable_when_active) {
                (false, _) => element.opacity(DIMMED_OPACITY),
                (true, true) => element,
                (true, false) => element
                    .cursor_pointer()
                    .when(!active, |element| {
                        element.hover(|style| style.text_color(palette.secondary))
                    })
                    .on_click(on_click),
            },
        )
        .child(label)
        .children(key.map(|key| key_label(key, &palette, scale)))
}
