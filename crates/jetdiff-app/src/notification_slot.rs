//! The notification slot above the keymap bar. A brief notification lies over the pinned one for a while.

use std::time::{Duration, Instant};

use crate::Notification;

pub const BRIEF_DURATION: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct NotificationSlot {
    pinned: Option<Notification>,
    brief: Option<(Notification, Instant)>,
}

impl NotificationSlot {
    pub fn show_brief(&mut self, notification: Notification, now: Instant) {
        self.brief = Some((notification, now + BRIEF_DURATION));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_lies_over_pinned() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.pin(Some(Notification::info("No changed files")));
        slot.show_brief(Notification::warning("Can't read"), start);
        assert_eq!(
            slot.current(start + Duration::from_secs(1)),
            Some(&Notification::warning("Can't read"))
        );
    }

    #[test]
    fn pinned_shows_again_after_3_seconds() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.pin(Some(Notification::info("No changed files")));
        slot.show_brief(Notification::warning("Can't read"), start);
        assert_eq!(
            slot.current(start + Duration::from_secs(3)),
            Some(&Notification::info("No changed files"))
        );
    }

    #[test]
    fn new_brief_replaces_previous() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.show_brief(Notification::info("Copied"), start);
        slot.show_brief(
            Notification::info("1 file changed"),
            start + Duration::from_secs(2),
        );
        assert_eq!(
            slot.current(start + Duration::from_secs(2)),
            Some(&Notification::info("1 file changed"))
        );
        assert_eq!(
            slot.current(start + Duration::from_secs(4)),
            Some(&Notification::info("1 file changed"))
        );
        assert_eq!(slot.current(start + Duration::from_secs(5)), None);
    }
}
