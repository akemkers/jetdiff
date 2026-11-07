//! A notification above the keymap bar. Whether it is pinned or brief is said by the field it comes in.

use jetdiff_viewmodel::theme::{Palette, Pill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
}

impl NotificationLevel {
    /// Info and Progress are on the surface, like the popups.
    pub fn colors<Color: Copy>(self, palette: &Palette<Color>) -> Pill<Color> {
        match self {
            Self::Info => Pill {
                background: palette.surface,
                text: palette.primary,
            },
            Self::Warning => palette.warning,
        }
    }
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

#[cfg(test)]
mod tests {
    use jetdiff_viewmodel::Theme;
    use jetdiff_viewmodel::theme::Pill;

    use super::*;

    #[test]
    fn info_is_on_the_surface_and_warning_has_colors_of_its_own() {
        let palette = Theme::built_in().palette;
        let surface = Pill {
            background: palette.surface,
            text: palette.primary,
        };
        assert_eq!(NotificationLevel::Info.colors(&palette), surface);
        assert_eq!(NotificationLevel::Warning.colors(&palette), palette.warning);
    }
}
