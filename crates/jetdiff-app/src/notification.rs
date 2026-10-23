//! A notification above the keymap bar. Whether it is pinned or brief is said by the field it comes in.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub text: String,
    pub level: NotificationLevel,
}

impl Notification {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: NotificationLevel::Info,
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: NotificationLevel::Warning,
        }
    }
}
