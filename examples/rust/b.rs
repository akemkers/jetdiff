//! A small tokenizer that merges equal characters.

use std::collections::HashMap;
use std::ops::Range;

/// A token with a byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Word,
    Number,
    Space,
    Punct,
}

const MAX_LEN: usize = 4096;

pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    for (i, c) in text.char_indices() {
        let kind = match c {
            '0'..='9' => Kind::Number,
            c if c.is_whitespace() => Kind::Space,
            c if c.is_ascii_punctuation() => Kind::Punct,
            _ => Kind::Word,
        };
        let end = i + c.len_utf8();
        match out.last_mut() {
            Some(last) if last.kind == kind => last.range.end = end,
            _ => out.push(Token { kind, range: i..end }),
        }
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
    fn numbers_are_merged() {
        assert_eq!(tokenize("12")[0].range, 0..2);
    }
}
