//! Keystrokes to `Command`. A small, vim-inspired keymap for reading. Not vim-compatible:
//! we pick keys that are easy to reach, also on a Norwegian keyboard. Only `g` waits for the next key.

use jetdiff_core::side::Side;

use crate::command::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// The character the key gives with shift and alt. With ctrl it is the base key.
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Tab,
    Enter,
    Escape,
    F(u8),
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl Key {
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            ctrl: false,
            shift: false,
            cmd: false,
        }
    }

    pub fn char(c: char) -> Self {
        Self::new(KeyCode::Char(c))
    }

    pub fn ctrl(c: char) -> Self {
        Self {
            ctrl: true,
            ..Self::char(c)
        }
    }

    pub fn shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parsed {
    Command(Command),
    /// The key is used without a command: the first `g` in `gg`, or `g` followed by something else.
    Pending,
    /// Not our key, e.g. Cmd shortcuts. Let GPUI actions take it.
    Ignored,
}

#[derive(Debug, Clone, Default)]
pub struct KeyParser {
    /// `g` is pressed, waiting for the second `g`.
    g: bool,
}

impl KeyParser {
    pub fn reset(&mut self) {
        self.g = false;
    }

    pub fn feed(&mut self, key: Key) -> Parsed {
        if key.cmd {
            return Parsed::Ignored;
        }
        if std::mem::take(&mut self.g) {
            return match key.code {
                KeyCode::Char('g') if !key.ctrl => Parsed::Command(Command::GoToTop),
                _ => Parsed::Pending,
            };
        }
        if key.code == KeyCode::Char('g') && !key.ctrl {
            self.g = true;
            return Parsed::Pending;
        }
        command(key).map_or(Parsed::Ignored, Parsed::Command)
    }
}

fn command(key: Key) -> Option<Command> {
    use Command::*;
    let cmd = match key.code {
        // The Ctrl variants are for muscle memory from vim. They are not in the keymap bar.
        KeyCode::Char(c) if key.ctrl => match c {
            'd' => HalfPageDown,
            'u' => HalfPageUp,
            'n' => NextHunk,
            'p' => PreviousHunk,
            _ => return None,
        },
        KeyCode::Char(c) => match c {
            '0' => ScrollToStart,
            '$' => ScrollToEnd,
            'j' => MoveDown,
            'k' => MoveUp,
            'h' => ScrollLeft,
            'l' => ScrollRight,
            'd' => HalfPageDown,
            'u' => HalfPageUp,
            'n' => NextHunk,
            'p' => PreviousHunk,
            'o' => FoldOpen,
            'f' => ToggleFolds,
            'y' => CopyCursorLocation,
            'r' => Rollback,
            'G' => GoToBottom,
            'H' => FocusSide(Side::Left),
            'L' => FocusSide(Side::Right),
            _ => return None,
        },
        KeyCode::Down => MoveDown,
        KeyCode::Up => MoveUp,
        KeyCode::Left => ScrollLeft,
        KeyCode::Right => ScrollRight,
        // F7 and Shift+F7 are IntelliJ's `NextDiff` and `PreviousDiff`
        KeyCode::F(7) if key.shift => PreviousHunk,
        KeyCode::F(7) => NextHunk,
        _ => return None,
    };
    Some(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Command::*;

    fn run(keys: &[Key]) -> Vec<Parsed> {
        let mut p = KeyParser::default();
        keys.iter().map(|&k| p.feed(k)).collect()
    }

    fn chars(s: &str) -> Vec<Key> {
        s.chars().map(Key::char).collect()
    }

    /// The last result for a sequence, and that everything before it waited.
    fn last(keys: &[Key]) -> Parsed {
        let out = run(keys);
        for p in &out[..out.len() - 1] {
            assert_eq!(*p, Parsed::Pending, "{keys:?}");
        }
        *out.last().unwrap()
    }

    fn c(cmd: Command) -> Parsed {
        Parsed::Command(cmd)
    }

    #[test]
    fn simple_keys() {
        assert_eq!(last(&chars("j")), c(MoveDown));
        assert_eq!(last(&chars("k")), c(MoveUp));
        assert_eq!(last(&chars("h")), c(ScrollLeft));
        assert_eq!(last(&chars("l")), c(ScrollRight));
        assert_eq!(last(&chars("0")), c(ScrollToStart));
        assert_eq!(last(&chars("$")), c(ScrollToEnd));
        assert_eq!(last(&chars("d")), c(HalfPageDown));
        assert_eq!(last(&chars("u")), c(HalfPageUp));
        assert_eq!(last(&chars("n")), c(NextHunk));
        assert_eq!(last(&chars("p")), c(PreviousHunk));
        assert_eq!(last(&chars("o")), c(FoldOpen));
        assert_eq!(last(&chars("f")), c(ToggleFolds));
        assert_eq!(last(&chars("y")), c(CopyCursorLocation));
        assert_eq!(last(&chars("r")), c(Rollback));
        assert_eq!(last(&chars("G")), c(GoToBottom));
        assert_eq!(last(&chars("H")), c(FocusSide(Side::Left)));
        assert_eq!(last(&chars("L")), c(FocusSide(Side::Right)));
        assert_eq!(last(&chars("gg")), c(GoToTop));
    }

    #[test]
    fn g_waits_for_next_key() {
        assert_eq!(
            run(&[Key::char('g'), Key::new(KeyCode::Other), Key::char('g')]),
            [Parsed::Pending, Parsed::Pending, Parsed::Pending]
        );
        assert_eq!(
            run(&chars("gxj")),
            [Parsed::Pending, Parsed::Pending, c(MoveDown)]
        );
        assert_eq!(run(&[Key::new(KeyCode::Other)]), [Parsed::Ignored]);
        assert_eq!(run(&chars("5")), [Parsed::Ignored]);
        assert_eq!(run(&chars("x")), [Parsed::Ignored]);
    }

    #[test]
    fn ctrl_and_cmd() {
        assert_eq!(last(&[Key::ctrl('d')]), c(HalfPageDown));
        assert_eq!(last(&[Key::ctrl('u')]), c(HalfPageUp));
        assert_eq!(last(&[Key::ctrl('n')]), c(NextHunk));
        assert_eq!(last(&[Key::ctrl('p')]), c(PreviousHunk));
        assert_eq!(run(&[Key::ctrl('x')]), [Parsed::Ignored]);
        let mut k = Key::char('d');
        k.cmd = true;
        assert_eq!(run(&[k]), [Parsed::Ignored]);
    }

    #[test]
    fn f7_and_arrow_keys() {
        let k = Key::new;
        assert_eq!(last(&[k(KeyCode::F(7))]), c(NextHunk));
        assert_eq!(last(&[k(KeyCode::F(7)).shift()]), c(PreviousHunk));
        assert_eq!(last(&[k(KeyCode::Down)]), c(MoveDown));
        assert_eq!(last(&[k(KeyCode::Up)]), c(MoveUp));
        assert_eq!(last(&[k(KeyCode::Left)]), c(ScrollLeft));
        assert_eq!(last(&[k(KeyCode::Right)]), c(ScrollRight));
        assert_eq!(run(&[k(KeyCode::F(5))]), [Parsed::Ignored]);
    }
}
