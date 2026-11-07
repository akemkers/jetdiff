//! The keymap bar with the most important keys, across the bottom of the window with the
//! notifications above it, and all the keys in a popup on `?`.

use gpui::{Div, Entity, FontWeight, actions, div, prelude::*, px};
use jetdiff_app::help::{ALL, KeymapBarKeys};

use crate::notification_slot_view::NotificationSlotView;
use crate::text::{FAMILY, SANS};
use crate::theme::{UiPalette, UiTheme};

actions!(jetdiff, [ToggleHelp, HideHelp]);

/// The bar keeps one height in both modes, so it doesn't wrap.
pub const KEYMAP_BAR_HEIGHT: f32 = 44.;
const KEYMAP_BAR_PADDING: f32 = 16.;
const KEYS_GAP: f32 = 16.;
const LABEL_GAP: f32 = 4.;
const TEXT_SIZE: f32 = 12.;
const KEY_LABEL_TEXT_SIZE: f32 = 11.;
const KEY_LABEL_PADDING: f32 = 5.;
const KEY_LABEL_RADIUS: f32 = 4.;

/// Sizes in the popup. Gives two columns with sections.
const KEYS_WIDTH: f32 = 110.;
const SECTION_WIDTH: f32 = 290.;
const SECTION_GAP: f32 = 32.;

/// The bar with the notification slot lying over whatever is above it.
pub fn keymap_bar(
    keys: &[KeymapBarKeys],
    notification_slot: Entity<NotificationSlotView>,
    palette: &UiPalette,
    scale: f32,
) -> Div {
    let s = |v: f32| px(v * scale);
    let bar = div()
        .h(s(KEYMAP_BAR_HEIGHT))
        .px(s(KEYMAP_BAR_PADDING))
        .flex()
        .items_center()
        .gap(s(KEYS_GAP))
        .overflow_hidden()
        .bg(palette.surface)
        .border_t_1()
        .border_color(palette.border)
        .font_family(SANS)
        .text_size(s(TEXT_SIZE))
        .text_color(palette.muted)
        .children(keys.iter().map(|&(labels, what)| {
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(s(LABEL_GAP))
                .children(labels.iter().map(|&label| key_label(label, palette, scale)))
                .child(div().ml(s(LABEL_GAP)).child(what))
        }));
    div()
        .relative()
        .flex_none()
        .w_full()
        .child(bar)
        .child(notification_slot)
}

pub(crate) fn key_label(key: &'static str, palette: &UiPalette, scale: f32) -> Div {
    let s = |v: f32| px(v * scale);
    div()
        .px(s(KEY_LABEL_PADDING))
        .bg(palette.key_label_background)
        .border_1()
        .border_color(palette.key_label_border)
        .rounded(s(KEY_LABEL_RADIUS))
        .font_family(FAMILY)
        .text_size(s(KEY_LABEL_TEXT_SIZE))
        .text_color(palette.secondary)
        .child(key)
}

pub struct KeyHelp {
    pub theme: UiTheme,
}

impl KeyHelp {
    /// Covers the parent and puts the popup in the middle.
    pub fn popup(&self) -> Div {
        let theme = self.theme;
        let sections = ALL.iter().map(|(title, keys)| {
            let rows = keys.iter().map(|&(keys, what)| {
                div()
                    .flex()
                    .child(
                        div()
                            .w(px(KEYS_WIDTH))
                            .text_color(theme.editor_fg)
                            .child(keys),
                    )
                    .child(what)
            });
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .w(px(SECTION_WIDTH))
                .child(
                    div()
                        .pb(px(2.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.editor_fg)
                        .child(*title),
                )
                .children(rows)
        });
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
                    .gap(px(14.))
                    .px(px(20.))
                    .py(px(16.))
                    .bg(theme.editor_bg)
                    .border_1()
                    .border_color(theme.line_number)
                    .rounded(px(6.))
                    .shadow_lg()
                    .font_family(FAMILY)
                    .text_size(px(13.))
                    .text_color(theme.line_number)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .max_w(px(2. * SECTION_WIDTH + SECTION_GAP))
                            .gap_x(px(SECTION_GAP))
                            .gap_y(px(14.))
                            .children(sections),
                    )
                    .child("? or Esc closes"),
            )
    }
}
