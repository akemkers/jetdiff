//! Beskjed over tastelinja. Den som vil vise noe, får entiteten.

use std::time::Duration;

use gpui::{Context, Render, SharedString, Task, Window, div, prelude::*, px};

use jetdiff_app::Notice;

use crate::settings::Settings;
use crate::text::FAMILY;

const DURATION: Duration = Duration::from_secs(3);

pub use jetdiff_app::Level;

type Text = (SharedString, Level);

#[derive(Default)]
pub struct Message {
    /// Står til den fjernes. En kort beskjed legger seg over den en stund.
    pinned: Option<Text>,
    brief: Option<Text>,
    /// En ny kort beskjed erstatter den, og den gamle timeren droppes.
    timer: Option<Task<()>>,
}

impl Message {
    /// Forsvinner av seg selv.
    pub fn show(&mut self, text: impl Into<SharedString>, level: Level, cx: &mut Context<Self>) {
        self.brief = Some((text.into(), level));
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DURATION).await;
            this.update(cx, |m, cx| {
                m.brief = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn pin(&mut self, text: impl Into<SharedString>, level: Level, cx: &mut Context<Self>) {
        self.pinned = Some((text.into(), level));
        cx.notify();
    }

    /// Fast beskjed fra `jetdiff-app`, eller ingen.
    pub fn pin_notice(&mut self, notice: Option<Notice>, cx: &mut Context<Self>) {
        match notice {
            Some(n) => self.pin(n.text, n.level, cx),
            None => self.unpin(cx),
        }
    }

    pub fn unpin(&mut self, cx: &mut Context<Self>) {
        self.pinned = None;
        cx.notify();
    }
}

impl Render for Message {
    /// Ligger over forelderen uten å ta plass, så innholdet under ikke hopper.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Settings>().theme;
        div().absolute().bottom_full().left_0().right_0().children(
            self.brief
                .clone()
                .or_else(|| self.pinned.clone())
                .map(|(text, level)| {
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .bg(match level {
                            Level::Info => theme.message_bg,
                            Level::Warning => theme.warning_bg,
                        })
                        .text_color(theme.message_fg)
                        .font_family(FAMILY)
                        .text_size(px(12.))
                        .child(text)
                }),
        )
    }
}
