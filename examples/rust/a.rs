//! A small tokenizer.

use std::collections::HashMap;

/// A token with a position.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Word,
    Number,
    Space,
}

const MAX_LEN: usize = 1024;

pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for (i, c) in text.char_indices() {
        let kind = if c.is_ascii_digit() {
            Kind::Number
        } else if c.is_whitespace() {
            Kind::Space
        } else {
            Kind::Word
        };
        out.push(Token { kind, start: i });
    }
    out
}

pub fn count(tokens: &[Token]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for t in tokens {
        *map.entry(format!("{:?}", t.kind)).or_insert(0) += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number() {
        assert_eq!(tokenize("1")[0].kind, Kind::Number);
    }
}
