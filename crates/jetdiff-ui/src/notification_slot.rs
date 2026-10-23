//! The notification slot above the keymap bar. Whoever wants to show a notification gets the entity.

use std::time::Duration;

use gpui::{Context, Render, Task, Window, div, prelude::*, px};

use jetdiff_app::{Notification, NotificationLevel};

use crate::settings::Settings;
use crate::text::FAMILY;

const BRIEF_DURATION: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct NotificationSlot {
    /// Stays until removed. A brief notification lies over it for a while.
    pinned: Option<Notification>,
    brief: Option<Notification>,
    /// A new brief notification replaces it, and the previous timer is dropped.
    timer: Option<Task<()>>,
}

impl NotificationSlot {
    /// Disappears by itself.
    pub fn show_brief(&mut self, notification: Notification, cx: &mut Context<Self>) {
        self.brief = Some(notification);
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BRIEF_DURATION).await;
            this.update(cx, |slot, cx| {
                slot.brief = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn pin(&mut self, notification: Option<Notification>, cx: &mut Context<Self>) {
        self.pinned = notification;
        cx.notify();
    }
}

impl Render for NotificationSlot {
    /// Lies over the parent without taking space, so the content below doesn't jump.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Settings>().theme;
        div().absolute().bottom_full().left_0().right_0().children(
            self.brief
                .clone()
                .or_else(|| self.pinned.clone())
                .map(|notification| {
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .bg(match notification.level {
                            NotificationLevel::Info => theme.notification_info_background,
                            NotificationLevel::Warning => theme.notification_warning_background,
                        })
                        .text_color(theme.notification_foreground)
                        .font_family(FAMILY)
                        .text_size(px(12.))
                        .child(notification.text)
                }),
        )
    }
}
