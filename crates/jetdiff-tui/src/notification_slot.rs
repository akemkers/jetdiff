//! The notification slot above the keymap bar. A brief notification lies over the pinned one for a while.

use std::time::{Duration, Instant};

use jetdiff_app::Notification;

const BRIEF_DURATION: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct NotificationSlot {
    pinned: Option<Notification>,
    brief: Option<(Notification, Instant)>,
}

impl NotificationSlot {
    pub fn show_brief(&mut self, notification: Notification) {
        self.brief = Some((notification, Instant::now() + BRIEF_DURATION));
    }

    pub fn pin(&mut self, notification: Option<Notification>) {
        self.pinned = notification;
    }

    pub fn current(&self, now: Instant) -> Option<&Notification> {
        match &self.brief {
            Some((notification, until)) if now < *until => Some(notification),
            _ => self.pinned.as_ref(),
        }
    }
}
