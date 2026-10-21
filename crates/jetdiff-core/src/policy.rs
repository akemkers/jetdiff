//! `ComparisonPolicy` with equality and hash per policy. Port of `ComparisonUtil.kt` and
//! the whitespace helpers in `StringsKmp.kt`.
//!
//! Whitespace is only `' '`, `'\t'` and `'\n'`, all ASCII. So equality and trimming can
//! work on UTF-8 bytes and give the same answer as Java on UTF-16 units. The hashes, however,
//! are computed on UTF-16 units like `String.hashCode`, so they are identical to IntelliJ's.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonPolicy {
    Default,
    TrimWhitespaces,
    IgnoreWhitespaces,
}

/// Lines with this few non-whitespace characters are "unimportant". `DiffConfig.UNIMPORTANT_LINE_CHAR_COUNT`.
pub const UNIMPORTANT_LINE_CHAR_COUNT: usize = 3;

pub fn is_space_enter_or_tab(c: char) -> bool {
    c == '\n' || c == '\t' || c == ' '
}

pub(crate) fn is_ws_byte(b: u8) -> bool {
    b == b'\n' || b == b'\t' || b == b' '
}

pub fn is_equals(text1: &str, text2: &str, policy: ComparisonPolicy) -> bool {
    match policy {
        ComparisonPolicy::Default => text1 == text2,
        ComparisonPolicy::TrimWhitespaces => equals_trim_whitespaces(text1, text2),
        ComparisonPolicy::IgnoreWhitespaces => equals_ignore_whitespaces(text1, text2),
    }
}

pub fn hash_code(text: &str, policy: ComparisonPolicy) -> i32 {
    match policy {
        ComparisonPolicy::Default => string_hash_code(text),
        ComparisonPolicy::TrimWhitespaces => string_hash_code(trim_whitespace(text)),
        ComparisonPolicy::IgnoreWhitespaces => string_hash_code_ignore_whitespaces(text),
    }
}

/// Like `is_equals`, but TRIM trims each line in a multi-line text.
pub fn is_equal_texts(text1: &str, text2: &str, policy: ComparisonPolicy) -> bool {
    match policy {
        ComparisonPolicy::Default => text1 == text2,
        ComparisonPolicy::TrimWhitespaces => equals_trim_whitespaces_per_line(text1, text2),
        ComparisonPolicy::IgnoreWhitespaces => equals_ignore_whitespaces(text1, text2),
    }
}

/// `ComparisonUtil.equalsTrimWhitespaces`: `"\nXY\n"` and `"\n XY \n"` are equal.
pub fn equals_trim_whitespaces_per_line(s1: &str, s2: &str) -> bool {
    let next_line = |s: &str, index: usize| match s[index..].find('\n') {
        Some(i) => (index + i + 1, false),
        None => (s.len(), true),
    };
    let (mut index1, mut index2) = (0, 0);
    loop {
        let (end1, last_line1) = next_line(s1, index1);
        let (end2, last_line2) = next_line(s2, index2);
        if last_line1 != last_line2 {
            return false;
        }
        if !equals_trim_whitespaces(&s1[index1..end1], &s2[index2..end2]) {
            return false;
        }
        index1 = end1;
        index2 = end2;
        if last_line1 {
            return true;
        }
    }
}

pub fn equals_ignore_whitespaces(text1: &str, text2: &str) -> bool {
    let (a, b) = (text1.as_bytes(), text2.as_bytes());
    let (len1, len2) = (a.len(), b.len());
    let (mut index1, mut index2) = (0, 0);
    while index1 < len1 && index2 < len2 {
        if a[index1] == b[index2] {
            index1 += 1;
            index2 += 1;
            continue;
        }

        let mut skipped = false;
        while index1 != len1 && is_ws_byte(a[index1]) {
            skipped = true;
            index1 += 1;
        }
        while index2 != len2 && is_ws_byte(b[index2]) {
            skipped = true;
            index2 += 1;
        }
        if !skipped {
            return false;
        }
    }

    a[index1..].iter().all(|&c| is_ws_byte(c)) && b[index2..].iter().all(|&c| is_ws_byte(c))
}

pub fn equals_trim_whitespaces(text1: &str, text2: &str) -> bool {
    trim_whitespace(text1) == trim_whitespace(text2)
}

fn trim_whitespace(text: &str) -> &str {
    text.trim_matches(is_space_enter_or_tab)
}

/// `String.hashCode` over UTF-16 units.
pub fn string_hash_code(text: &str) -> i32 {
    hash_chars(text.chars())
}

pub fn string_hash_code_ignore_whitespaces(text: &str) -> i32 {
    hash_chars(text.chars().filter(|&c| !is_space_enter_or_tab(c)))
}

fn hash_chars(chars: impl Iterator<Item = char>) -> i32 {
    let mut h = 0i32;
    let mut buf = [0u16; 2];
    for c in chars {
        for &unit in c.encode_utf16(&mut buf).iter() {
            h = h.wrapping_mul(31).wrapping_add(unit as i32);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_equals_java() {
        assert_eq!(string_hash_code(""), 0);
        assert_eq!(string_hash_code("abc"), 96354);
        // "Hello, World!".hashCode() in Java
        assert_eq!(string_hash_code("Hello, World!"), 1498789909);
        // Surrogate pairs count as two units: "😀".hashCode()
        assert_eq!(string_hash_code("😀"), 0xD83D * 31 + 0xDE00);
    }

    #[test]
    fn ignore_whitespaces() {
        assert!(equals_ignore_whitespaces("a b", "ab"));
        assert!(equals_ignore_whitespaces("  a\tb  ", "ab"));
        assert!(equals_ignore_whitespaces("", " \t"));
        assert!(!equals_ignore_whitespaces("ab", "ba"));
        assert!(!equals_ignore_whitespaces("a", "a b"));
        // '\r' is not whitespace here
        assert!(!equals_ignore_whitespaces("a\r", "a"));
        assert!(equals_ignore_whitespaces("æ ø", "æø"));
        assert_eq!(
            string_hash_code_ignore_whitespaces(" a b "),
            string_hash_code("ab")
        );
    }

    #[test]
    fn trim_whitespaces() {
        assert!(equals_trim_whitespaces("  ab\t", "ab"));
        assert!(!equals_trim_whitespaces("a b", "ab"));
        assert_eq!(
            hash_code(" ab ", ComparisonPolicy::TrimWhitespaces),
            string_hash_code("ab")
        );
    }

    #[test]
    fn trim_per_line() {
        assert!(equals_trim_whitespaces_per_line("\nXY\n", "\n XY \n"));
        assert!(!equals_trim_whitespaces_per_line("\nXY\n", "\nX Y\n"));
        assert!(!equals_trim_whitespaces_per_line("a\nb", "a b"));
        assert!(!equals_trim_whitespaces_per_line("a\n", "a"));
        assert!(equals_trim_whitespaces_per_line("", "  "));
        assert!(equals_trim_whitespaces_per_line("a \n b", "a\nb"));
    }
}
