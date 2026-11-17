//! The keymap bar with the most important keys, across the bottom of the window with the
//! notifications above it, and all the keys in a popup on `?`.

use gpui::{Div, Entity, FontWeight, actions, div, prelude::*, px};
use jetdiff_app::help::{ALL, Keys};

use crate::notification_slot_view::NotificationSlotView;
use crate::text::{FAMILY, SANS};
use crate::theme::{UiPalette, UiTheme};

actions!(jetdiff, [ToggleHelp, HideHelp]);

/// The bar keeps one height in both modes, so it doesn't wrap.
pub const KEYMAP_BAR_HEIGHT: f32 = 48.;
const KEYMAP_BAR_PADDING: f32 = 16.;
const KEYS_GAP: f32 = 16.;
const LABEL_GAP: f32 = 4.;
const TEXT_SIZE: f32 = 14.;
const KEY_LABEL_TEXT_SIZE: f32 = 13.;
const KEY_LABEL_PADDING: f32 = 6.;
const KEY_LABEL_RADIUS: f32 = 4.;

/// Sizes in the popup. Gives two columns with sections.
const KEYS_WIDTH: f32 = 110.;
const SECTION_WIDTH: f32 = 290.;
const SECTION_GAP: f32 = 32.;
const ROW_GAP: f32 = 2.;
const TITLE_PADDING: f32 = 2.;
const POPUP_GAP: f32 = 14.;
const POPUP_HORIZONTAL_PADDING: f32 = 20.;
const POPUP_VERTICAL_PADDING: f32 = 16.;
const POPUP_RADIUS: f32 = 6.;
const POPUP_TEXT_SIZE: f32 = 13.;

/// The bar with the notification slot lying over whatever is above it.
pub fn keymap_bar(
    keys: &[Keys],
    notification_slot: Entity<NotificationSlotView>,
    palette: &UiPalette,
    scale: f32,
) -> Div {
    let scaled = |size: f32| px(size * scale);
    let bar = div()
        .h(scaled(KEYMAP_BAR_HEIGHT))
        .px(scaled(KEYMAP_BAR_PADDING))
        .flex()
        .items_center()
        .gap(scaled(KEYS_GAP))
        .overflow_hidden()
        .bg(palette.surface)
        .border_t_1()
        .border_color(palette.border)
        .font_family(SANS)
        .text_size(scaled(TEXT_SIZE))
        .text_color(palette.muted)
        .children(keys.iter().map(|&(labels, what)| {
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(scaled(LABEL_GAP))
                .children(labels.iter().map(|&label| key_label(label, palette, scale)))
                .child(div().ml(scaled(LABEL_GAP)).child(what))
        }));
    div()
        .relative()
        .flex_none()
        .w_full()
        .child(bar)
        .child(notification_slot)
}

pub(crate) fn key_label(key: &'static str, palette: &UiPalette, scale: f32) -> Div {
    let scaled = |size: f32| px(size * scale);
    div()
        .px(scaled(KEY_LABEL_PADDING))
        .bg(palette.key_label_background)
        .border_1()
        .border_color(palette.key_label_border)
        .rounded(scaled(KEY_LABEL_RADIUS))
        .font_family(FAMILY)
        .text_size(scaled(KEY_LABEL_TEXT_SIZE))
        .text_color(palette.secondary)
        .child(key)
}

pub struct KeyHelp {
    pub theme: UiTheme,
    pub scale: f32,
}

impl KeyHelp {
    /// Covers the parent and puts the popup in the middle.
    pub fn popup(&self) -> Div {
        let palette = self.theme.palette;
        let scale = self.scale;
        let s = |v: f32| px(v * scale);
        let sections = ALL
            .iter()
            .map(|&(title, keys)| section(title, keys, &palette, scale));
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(s(POPUP_GAP))
                    .px(s(POPUP_HORIZONTAL_PADDING))
                    .py(s(POPUP_VERTICAL_PADDING))
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .rounded(s(POPUP_RADIUS))
                    .shadow_lg()
                    .font_family(FAMILY)
                    .text_size(s(POPUP_TEXT_SIZE))
                    .text_color(palette.muted)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .max_w(s(2. * SECTION_WIDTH + SECTION_GAP))
                            .gap_x(s(SECTION_GAP))
                            .gap_y(s(POPUP_GAP))
                            .children(sections),
                    )
                    .child("? or Esc closes"),
            )
    }
}

fn section(title: &'static str, keys: &[Keys], palette: &UiPalette, scale: f32) -> Div {
    let s = |v: f32| px(v * scale);
    let rows = keys.iter().map(|&(labels, what)| {
        div()
            .flex()
            .items_center()
            .child(
                div()
                    .w(s(KEYS_WIDTH))
                    .flex()
                    .gap(s(LABEL_GAP))
                    .children(labels.iter().map(|&label| key_label(label, palette, scale))),
            )
            .child(what)
    });
    div()
        .flex()
        .flex_col()
        .gap(s(ROW_GAP))
        .w(s(SECTION_WIDTH))
        .child(
            div()
                .pb(s(TITLE_PADDING))
                .font_weight(FontWeight::BOLD)
                .text_color(palette.primary)
                .child(title),
        )
        .children(rows)
}
