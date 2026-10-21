//! GPUI key to `Key` from `jetdiff-viewmodel`, which translates it further to `Command`.

use gpui::Keystroke;
use jetdiff_viewmodel::{Key, KeyCode};

/// `key_char` includes shift and alt, so `G`, `$` and `[` on a Norwegian keyboard come out right. With
/// ctrl, `key_char` is empty, and `key` is the base key.
pub fn to_key(keystroke: &Keystroke) -> Key {
    let modifiers = keystroke.modifiers;
    let single = |text: &str| {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(character), None) => Some(character),
            _ => None,
        }
    };
    let code = match keystroke.key.as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "tab" => KeyCode::Tab,
        "enter" => KeyCode::Enter,
        "escape" => KeyCode::Escape,
        name if let Some(number) = name
            .strip_prefix('f')
            .and_then(|digits| digits.parse().ok()) =>
        {
            KeyCode::F(number)
        }
        key => {
            let typed = if modifiers.control {
                None
            } else {
                keystroke.key_char.as_deref().and_then(single)
            };
            match typed.or_else(|| single(key)) {
                Some(character) => KeyCode::Char(character),
                None => KeyCode::Other,
            }
        }
    };
    Key {
        code,
        ctrl: modifiers.control,
        shift: modifiers.shift,
        cmd: modifiers.platform,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn stroke(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.into(),
            key_char: key_char.map(Into::into),
        }
    }

    #[test]
    fn keys_from_gpui() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..none
        };
        let ctrl = Modifiers {
            control: true,
            ..none
        };
        let alt = Modifiers { alt: true, ..none };
        assert_eq!(
            to_key(&stroke("g", Some("G"), shift)).code,
            KeyCode::Char('G')
        );
        assert_eq!(to_key(&stroke("d", None, ctrl)), Key::ctrl('d'));
        // Norwegian Alt+8 gives `[`
        assert_eq!(
            to_key(&stroke("8", Some("["), alt)).code,
            KeyCode::Char('[')
        );
        assert_eq!(to_key(&stroke("f7", None, shift)).code, KeyCode::F(7));
        assert_eq!(
            to_key(&stroke("f", Some("f"), none)).code,
            KeyCode::Char('f')
        );
        assert_eq!(to_key(&stroke("tab", None, none)).code, KeyCode::Tab);
        assert_eq!(to_key(&stroke("home", None, none)).code, KeyCode::Other);
        let cmd = to_key(&stroke(
            "=",
            None,
            Modifiers {
                platform: true,
                ..none
            },
        ));
        assert!(cmd.cmd);
    }
}
