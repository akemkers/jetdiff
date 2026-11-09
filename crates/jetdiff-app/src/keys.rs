//! The keys outside the diff. The diff's own keys are `KeyParser` in `jetdiff-viewmodel`.

use jetdiff_viewmodel::{Key, KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FocusLeft,
    FocusDown,
    FocusUp,
    FocusRight,
    ToggleSidePanel,
    ToggleDiffMode,
    NextFile,
    PrevFile,
    NextCommit,
    /// The walkthrough, in and out.
    ToggleWalkthrough,
    PrevCommit,
    SourceReload,
    ToggleHelp,
    HideHelp,
    Quit,
}

/// In GPUI's syntax. `secondary` is Cmd on macOS and Ctrl elsewhere. GPUI matches on the character that
/// is typed, so Plus is `+` on Norwegian and `=` on US.
pub const BINDINGS: &[(&str, AppCommand)] = &[
    ("secondary-+", AppCommand::ZoomIn),
    ("secondary-=", AppCommand::ZoomIn),
    ("secondary--", AppCommand::ZoomOut),
    ("secondary-0", AppCommand::ResetZoom),
    ("ctrl-h", AppCommand::FocusLeft),
    ("ctrl-j", AppCommand::FocusDown),
    ("ctrl-k", AppCommand::FocusUp),
    ("ctrl-l", AppCommand::FocusRight),
    ("b", AppCommand::ToggleSidePanel),
    ("v", AppCommand::ToggleDiffMode),
    ("tab", AppCommand::NextFile),
    ("J", AppCommand::NextFile),
    ("shift-tab", AppCommand::PrevFile),
    ("K", AppCommand::PrevFile),
    ("N", AppCommand::NextCommit),
    ("w", AppCommand::ToggleWalkthrough),
    ("P", AppCommand::PrevCommit),
    ("r", AppCommand::SourceReload),
    ("?", AppCommand::ToggleHelp),
    ("escape", AppCommand::HideHelp),
    ("q", AppCommand::Quit),
    ("secondary-q", AppCommand::Quit),
    ("secondary-w", AppCommand::Quit),
];

pub fn app_command(key: Key) -> Option<AppCommand> {
    BINDINGS
        .iter()
        .find(|(binding, _)| parse(binding).is_some_and(|b| matches(b, key)))
        .map(|&(_, cmd)| cmd)
}

fn parse(binding: &str) -> Option<Key> {
    let (modifiers, name) = match binding.strip_suffix("--") {
        Some(modifiers) => (modifiers, "-"),
        None => binding.rsplit_once('-').unwrap_or(("", binding)),
    };
    let code = match name {
        "tab" => KeyCode::Tab,
        "enter" => KeyCode::Enter,
        "escape" => KeyCode::Escape,
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
    };
    let mut key = Key::new(code);
    for modifier in modifiers.split('-').filter(|m| !m.is_empty()) {
        match modifier {
            "ctrl" => key.ctrl = true,
            "shift" => key.shift = true,
            "cmd" => key.cmd = true,
            "secondary" if cfg!(target_os = "macos") => key.cmd = true,
            "secondary" => key.ctrl = true,
            _ => return None,
        }
    }
    Some(key)
}

/// Shift is already in the character, like `J`.
fn matches(binding: Key, key: Key) -> bool {
    let shift = matches!(key.code, KeyCode::Char(_)) || binding.shift == key.shift;
    binding.code == key.code && binding.ctrl == key.ctrl && binding.cmd == key.cmd && shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bindings_can_be_read() {
        for (binding, _) in BINDINGS {
            assert!(parse(binding).is_some(), "{binding}");
        }
    }

    #[test]
    fn keys_to_commands() {
        let secondary = |c| {
            let mut k = Key::char(c);
            if cfg!(target_os = "macos") {
                k.cmd = true;
            } else {
                k.ctrl = true;
            }
            k
        };
        assert_eq!(
            app_command(Key::char('J').shift()),
            Some(AppCommand::NextFile)
        );
        assert_eq!(
            app_command(Key::new(KeyCode::Tab)),
            Some(AppCommand::NextFile)
        );
        assert_eq!(
            app_command(Key::new(KeyCode::Tab).shift()),
            Some(AppCommand::PrevFile)
        );
        assert_eq!(
            app_command(Key::char('N').shift()),
            Some(AppCommand::NextCommit)
        );
        assert_eq!(app_command(Key::char('n')), None);
        assert_eq!(app_command(Key::char('r')), Some(AppCommand::SourceReload));
        assert_eq!(app_command(Key::char('z')), None);
        assert_eq!(app_command(Key::ctrl('h')), Some(AppCommand::FocusLeft));
        assert_eq!(app_command(secondary('-')), Some(AppCommand::ZoomOut));
        assert_eq!(app_command(secondary('+')), Some(AppCommand::ZoomIn));
        assert_eq!(
            app_command(Key::char('?').shift()),
            Some(AppCommand::ToggleHelp)
        );
        assert_eq!(app_command(Key::char('j')), None);
        assert_eq!(app_command(Key::ctrl('d')), None);
    }
}
