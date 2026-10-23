//! The notification slot above the keymap bar. Whoever wants to show a notification gets the entity.

use std::time::Instant;

use gpui::{Context, Render, Task, Window, div, prelude::*, px};

use jetdiff_app::notification_slot::BRIEF_DURATION;
use jetdiff_app::{Notification, NotificationLevel, NotificationSlot};

use crate::settings::Settings;
use crate::text::FAMILY;

const HORIZONTAL_PADDING: f32 = 10.;
const VERTICAL_PADDING: f32 = 4.;
const TEXT_SIZE: f32 = 12.;

#[derive(Default)]
pub struct NotificationSlotView {
    slot: NotificationSlot,
    /// Repaints when the brief notification expires. A new brief notification drops the previous one.
    repaint: Option<Task<()>>,
}

impl NotificationSlotView {
    pub fn show_brief(&mut self, notification: Notification, cx: &mut Context<Self>) {
        self.slot.show_brief(notification, Instant::now());
        self.repaint = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BRIEF_DURATION).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        }));
        cx.notify();
    }

    pub fn pin(&mut self, notification: Option<Notification>, cx: &mut Context<Self>) {
        self.slot.pin(notification);
        cx.notify();
    }
}

impl Render for NotificationSlotView {
    /// Lies over the parent without taking space, so the content below doesn't jump.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Settings>().theme;
        div().absolute().bottom_full().left_0().right_0().children(
            self.slot
                .current(Instant::now())
                .cloned()
                .map(|notification| {
                    div()
                        .px(px(HORIZONTAL_PADDING))
                        .py(px(VERTICAL_PADDING))
                        .bg(match notification.level {
                            NotificationLevel::Info => theme.notification_info_background,
                            NotificationLevel::Warning => theme.notification_warning_background,
                        })
                        .text_color(theme.notification_foreground)
                        .font_family(FAMILY)
                        .text_size(px(TEXT_SIZE))
                        .child(notification.text)
                }),
        )
    }
}
