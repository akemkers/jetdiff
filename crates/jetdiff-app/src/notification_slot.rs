//! The notification slot above the keymap bar. A brief notification lies over the pinned one for a
//! while. A progress, about something going on in the background, lies between them.

use std::time::{Duration, Instant};

use crate::Notification;

pub const BRIEF_DURATION: Duration = Duration::from_secs(3);

/// Something going on in the background, like a walkthrough being written. SlotContent with a spinner and
/// the time it has taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub text: String,
    pub started: Instant,
}

impl Progress {
    pub fn new(text: impl Into<String>, started: Instant) -> Self {
        Self {
            text: text.into(),
            started,
        }
    }

    /// Like `1:12`.
    pub fn elapsed(&self, now: Instant) -> String {
        let seconds = now.saturating_duration_since(self.started).as_secs();
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// What the slot shows now.
#[derive(Debug, PartialEq, Eq)]
pub enum SlotContent<'a> {
    Notification(&'a Notification),
    Progress(&'a Progress),
}

#[derive(Default)]
pub struct NotificationSlot {
    pinned: Option<Notification>,
    progress: Option<Progress>,
    brief: Option<(Notification, Instant)>,
}

impl NotificationSlot {
    pub fn show_brief(&mut self, notification: Notification, now: Instant) {
        self.brief = Some((notification, now + BRIEF_DURATION));
    }

    pub fn pin(&mut self, notification: Option<Notification>) {
        self.pinned = notification;
    }

    /// `None` when it is done.
    pub fn show_progress(&mut self, progress: Option<Progress>) {
        self.progress = progress;
    }

    pub fn current(&self, now: Instant) -> Option<SlotContent<'_>> {
        match &self.brief {
            Some((notification, until)) if now < *until => {
                Some(SlotContent::Notification(notification))
            }
            _ => match &self.progress {
                Some(progress) => Some(SlotContent::Progress(progress)),
                None => self.pinned.as_ref().map(SlotContent::Notification),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the slot shows, with the time a progress has taken.
    fn shown(slot: &NotificationSlot, now: Instant) -> Option<String> {
        slot.current(now).map(|shown| match shown {
            SlotContent::Notification(notification) => notification.text.clone(),
            SlotContent::Progress(progress) => {
                format!("{} {}", progress.text, progress.elapsed(now))
            }
        })
    }

    #[test]
    fn brief_lies_over_pinned() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.pin(Some(Notification::info("No changed files")));
        slot.show_brief(Notification::warning("Can't read"), start);
        assert_eq!(
            shown(&slot, start + Duration::from_secs(1)).as_deref(),
            Some("Can't read")
        );
    }

    #[test]
    fn pinned_shows_again_after_3_seconds() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.pin(Some(Notification::info("No changed files")));
        slot.show_brief(Notification::warning("Can't read"), start);
        assert_eq!(
            shown(&slot, start + Duration::from_secs(3)).as_deref(),
            Some("No changed files")
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
            shown(&slot, start + Duration::from_secs(2)).as_deref(),
            Some("1 file changed")
        );
        assert_eq!(
            shown(&slot, start + Duration::from_secs(4)).as_deref(),
            Some("1 file changed")
        );
        assert_eq!(shown(&slot, start + Duration::from_secs(5)), None);
    }

    #[test]
    fn progress_lies_under_brief_and_over_pinned() {
        let start = Instant::now();
        let mut slot = NotificationSlot::default();
        slot.pin(Some(Notification::info("No changed files")));
        slot.show_progress(Some(Progress::new("Writing", start)));
        let at = |seconds| start + Duration::from_secs(seconds);
        assert_eq!(shown(&slot, at(72)).as_deref(), Some("Writing 1:12"));
        slot.show_brief(Notification::info("Copied"), at(72));
        assert_eq!(shown(&slot, at(73)).as_deref(), Some("Copied"));
        assert_eq!(shown(&slot, at(80)).as_deref(), Some("Writing 1:20"));
        slot.show_progress(None);
        assert_eq!(shown(&slot, at(80)).as_deref(), Some("No changed files"));
    }
}
