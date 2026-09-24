//! Tastetrykk til `Command`. Et lite, vim-inspirert keymap for lesing. Ikke vim-kompatibelt:
//! vi velger taster som er lette å nå, også på norsk tastatur. Kun `g` venter på neste tast.

use crate::command::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Tegnet tasten gir med shift og alt. Med ctrl er det grunntasten.
    Char(char),
    Esc,
    Tab,
    Up,
    Down,
    Left,
    Right,
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
    /// Tasten er brukt uten kommando: første `g` i `gg`, eller `g` fulgt av noe annet.
    Pending,
    /// Ikke vår tast, f.eks. Cmd-snarveier. La GPUI-actions ta den.
    Ignored,
}

#[derive(Debug, Clone, Default)]
pub struct KeyParser {
    /// `g` er trykket, venter på `g` nummer to.
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
        // Ctrl-variantene er for muskelminnet fra vim. De står ikke i tastelinja.
        KeyCode::Char(c) if key.ctrl => match c {
            'd' => HalfPageDown,
            'u' => HalfPageUp,
            'n' => NextChange,
            'p' => PrevChange,
            _ => return None,
        },
        KeyCode::Char(c) => match c {
            '0' => LineStart,
            '$' => LineEnd,
            'j' => MoveDown,
            'k' => MoveUp,
            'h' => MoveLeft,
            'l' => MoveRight,
            'd' => HalfPageDown,
            'u' => HalfPageUp,
            'n' => NextChange,
            'p' => PrevChange,
            'o' => FoldOpen,
            'f' => ToggleFolds,
            'q' => Quit,
            'G' => GoToBottom,
            _ => return None,
        },
        KeyCode::Tab => SwitchSide,
        KeyCode::Down => MoveDown,
        KeyCode::Up => MoveUp,
        KeyCode::Left => MoveLeft,
        KeyCode::Right => MoveRight,
        // F7 og Shift+F7 er IntelliJ sine `NextDiff` og `PreviousDiff`
        KeyCode::F(7) if key.shift => PrevChange,
        KeyCode::F(7) => NextChange,
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

    /// Siste resultat for en sekvens, og at alt før ventet.
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
    fn enkle_taster() {
        assert_eq!(last(&chars("j")), c(MoveDown));
        assert_eq!(last(&chars("k")), c(MoveUp));
        assert_eq!(last(&chars("h")), c(MoveLeft));
        assert_eq!(last(&chars("l")), c(MoveRight));
        assert_eq!(last(&chars("0")), c(LineStart));
        assert_eq!(last(&chars("$")), c(LineEnd));
        assert_eq!(last(&chars("d")), c(HalfPageDown));
        assert_eq!(last(&chars("u")), c(HalfPageUp));
        assert_eq!(last(&chars("n")), c(NextChange));
        assert_eq!(last(&chars("p")), c(PrevChange));
        assert_eq!(last(&chars("o")), c(FoldOpen));
        assert_eq!(last(&chars("f")), c(ToggleFolds));
        assert_eq!(last(&chars("q")), c(Quit));
        assert_eq!(last(&chars("G")), c(GoToBottom));
        assert_eq!(last(&chars("gg")), c(GoToTop));
        assert_eq!(last(&[Key::new(KeyCode::Tab)]), c(SwitchSide));
    }

    #[test]
    fn g_venter_paa_neste_tast() {
        assert_eq!(
            run(&[Key::char('g'), Key::new(KeyCode::Esc), Key::char('g')]),
            [Parsed::Pending, Parsed::Pending, Parsed::Pending]
        );
        assert_eq!(
            run(&chars("gxj")),
            [Parsed::Pending, Parsed::Pending, c(MoveDown)]
        );
        assert_eq!(run(&[Key::new(KeyCode::Esc)]), [Parsed::Ignored]);
        assert_eq!(run(&chars("5")), [Parsed::Ignored]);
        assert_eq!(run(&chars("x")), [Parsed::Ignored]);
    }

    #[test]
    fn ctrl_og_cmd() {
        assert_eq!(last(&[Key::ctrl('d')]), c(HalfPageDown));
        assert_eq!(last(&[Key::ctrl('u')]), c(HalfPageUp));
        assert_eq!(last(&[Key::ctrl('n')]), c(NextChange));
        assert_eq!(last(&[Key::ctrl('p')]), c(PrevChange));
        assert_eq!(run(&[Key::ctrl('x')]), [Parsed::Ignored]);
        let mut k = Key::char('d');
        k.cmd = true;
        assert_eq!(run(&[k]), [Parsed::Ignored]);
    }

    #[test]
    fn f7_og_piltaster() {
        let k = Key::new;
        assert_eq!(last(&[k(KeyCode::F(7))]), c(NextChange));
        assert_eq!(last(&[k(KeyCode::F(7)).shift()]), c(PrevChange));
        assert_eq!(last(&[k(KeyCode::Down)]), c(MoveDown));
        assert_eq!(last(&[k(KeyCode::Up)]), c(MoveUp));
        assert_eq!(last(&[k(KeyCode::Left)]), c(MoveLeft));
        assert_eq!(last(&[k(KeyCode::Right)]), c(MoveRight));
        assert_eq!(run(&[k(KeyCode::F(5))]), [Parsed::Ignored]);
    }
}
