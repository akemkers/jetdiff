//! Crossterm key to `Key` from `jetdiff-viewmodel`.

use jetdiff_viewmodel::{Key, KeyCode};
use ratatui::crossterm::event::{KeyCode as Code, KeyEvent, KeyModifiers};

pub fn to_key(event: KeyEvent) -> Key {
    let modifiers = event.modifiers;
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let code = match event.code {
        Code::Char(ch) => KeyCode::Char(ch),
        Code::Up => KeyCode::Up,
        Code::Down => KeyCode::Down,
        Code::Left => KeyCode::Left,
        Code::Right => KeyCode::Right,
        Code::Tab => KeyCode::Tab,
        // Shift-Tab comes as its own code without shift in all terminals
        Code::BackTab => {
            return Key {
                shift: true,
                ..Key::new(KeyCode::Tab)
            };
        }
        Code::Enter => KeyCode::Enter,
        Code::Esc => KeyCode::Escape,
        Code::F(number) => KeyCode::F(number),
        _ => KeyCode::Other,
    };
    Key {
        code,
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        shift,
        cmd: modifiers.contains(KeyModifiers::SUPER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: Code, m: KeyModifiers) -> Key {
        to_key(KeyEvent::new(code, m))
    }

    #[test]
    fn keys_from_crossterm() {
        assert_eq!(
            key(Code::Char('G'), KeyModifiers::SHIFT).code,
            KeyCode::Char('G')
        );
        assert_eq!(key(Code::Char('d'), KeyModifiers::CONTROL), Key::ctrl('d'));
        assert_eq!(
            key(Code::BackTab, KeyModifiers::SHIFT),
            Key::new(KeyCode::Tab).shift()
        );
        assert_eq!(key(Code::Esc, KeyModifiers::NONE).code, KeyCode::Escape);
        assert_eq!(key(Code::Home, KeyModifiers::NONE).code, KeyCode::Other);
    }
}
