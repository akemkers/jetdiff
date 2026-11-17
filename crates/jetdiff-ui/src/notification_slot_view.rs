//! The notification slot above the keymap bar. Whoever wants to show a notification gets the entity.
//! A progress is shown with a spinner and the time it has taken.

use std::time::{Duration, Instant};

use gpui::{Context, Render, Task, Window, div, prelude::*, px};

use jetdiff_app::notification_slot::BRIEF_DURATION;
use jetdiff_app::{Notification, NotificationLevel, NotificationSlot, Progress, SlotContent};

use crate::settings::Settings;
use crate::text::FAMILY;

const HORIZONTAL_PADDING: f32 = 10.;
const VERTICAL_PADDING: f32 = 4.;
const TEXT_SIZE: f32 = 12.;
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// How long each spinner frame shows. Also how often a progress is drawn again.
const SPINNER_FRAME: Duration = Duration::from_millis(100);

pub struct NotificationSlotView {
    slot: NotificationSlot,
    /// Repaints when the brief notification expires. A new brief notification drops the previous one.
    repaint: Option<Task<()>>,
    /// Moves the spinner while there is a progress.
    spin: Option<Task<()>>,
    /// Follows the zoom in the diff.
    scale: f32,
}

impl Default for NotificationSlotView {
    fn default() -> Self {
        Self {
            slot: Default::default(),
            repaint: None,
            spin: None,
            scale: 1.,
        }
    }
}

impl NotificationSlotView {
    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

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

    /// `None` when it is done.
    pub fn show_progress(&mut self, progress: Option<Progress>, cx: &mut Context<Self>) {
        self.spin = progress.is_some().then(|| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(SPINNER_FRAME).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });
        self.slot.show_progress(progress);
        cx.notify();
    }
}

impl Render for NotificationSlotView {
    /// Lies over the parent without taking space, so the content below doesn't jump.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.global::<Settings>().theme.palette;
        let scaled = |size: f32| px(size * self.scale);
        let now = Instant::now();
        let shown = self.slot.current(now).map(|content| match content {
            SlotContent::Notification(notification) => {
                (notification.level, notification.text.clone())
            }
            SlotContent::Progress(progress) => {
                let since = now.saturating_duration_since(progress.started);
                let frame = (since.as_millis() / SPINNER_FRAME.as_millis()) as usize;
                let spinner = SPINNER[frame % SPINNER.len()];
                let text = format!("{spinner} {} {}", progress.text, progress.elapsed(now));
                (NotificationLevel::Info, text)
            }
        });
        div()
            .absolute()
            .bottom_full()
            .left_0()
            .right_0()
            .children(shown.map(|(level, text)| {
                let colors = level.colors(&palette);
                div()
                    .px(scaled(HORIZONTAL_PADDING))
                    .py(scaled(VERTICAL_PADDING))
                    .bg(colors.background)
                    .border_t_1()
                    .border_color(palette.border)
                    .text_color(colors.text)
                    .font_family(FAMILY)
                    .text_size(scaled(TEXT_SIZE))
                    .child(text)
            }))
    }
}
