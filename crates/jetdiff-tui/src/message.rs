//! Beskjed over tastelinja. En kort beskjed legger seg over den faste en stund.

use std::time::{Duration, Instant};

use jetdiff_app::{Level, Notice};

const DURATION: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct Message {
    pinned: Option<Notice>,
    brief: Option<(Notice, Instant)>,
}

impl Message {
    pub fn show(&mut self, text: impl Into<String>, level: Level) {
        let notice = Notice {
            text: text.into(),
            level,
        };
        self.brief = Some((notice, Instant::now() + DURATION));
    }

    pub fn pin(&mut self, notice: Option<Notice>) {
        self.pinned = notice;
    }

    pub fn current(&self, now: Instant) -> Option<&Notice> {
        match &self.brief {
            Some((notice, until)) if now < *until => Some(notice),
            _ => self.pinned.as_ref(),
        }
    }
}
