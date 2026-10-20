//! Crossterm-tast til `Key` fra `jetdiff-viewmodel`.

use jetdiff_viewmodel::{Key, KeyCode};
use ratatui::crossterm::event::{KeyCode as Code, KeyEvent, KeyModifiers};

pub fn to_key(event: KeyEvent) -> Key {
    let m = event.modifiers;
    let shift = m.contains(KeyModifiers::SHIFT);
    let code = match event.code {
        Code::Char(c) => KeyCode::Char(c),
        Code::Up => KeyCode::Up,
        Code::Down => KeyCode::Down,
        Code::Left => KeyCode::Left,
        Code::Right => KeyCode::Right,
        Code::Tab => KeyCode::Tab,
        // Shift-Tab kommer som egen kode uten shift i alle terminaler
        Code::BackTab => {
            return Key {
                shift: true,
                ..Key::new(KeyCode::Tab)
            };
        }
        Code::Enter => KeyCode::Enter,
        Code::Esc => KeyCode::Escape,
        Code::F(n) => KeyCode::F(n),
        _ => KeyCode::Other,
    };
    Key {
        code,
        ctrl: m.contains(KeyModifiers::CONTROL),
        shift,
        cmd: m.contains(KeyModifiers::SUPER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: Code, m: KeyModifiers) -> Key {
        to_key(KeyEvent::new(code, m))
    }

    #[test]
    fn taster_fra_crossterm() {
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
