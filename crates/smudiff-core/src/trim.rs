//! Port av delene av `TrimUtil.kt` som trengs. Argumentrekkefølgen `(start1, start2, end1, end2)`
//! er beholdt fra originalen. Tekst-varianter tar byte-offsets, whitespace er alltid ASCII.
//!
//! Tegnklassene bruker kodepunkter som Java. Tegnsetting og whitespace er ASCII, så
//! byte-for-byte på UTF-8 gir samme svar som Java på UTF-16-enheter.

use crate::character_utils::{
    is_alphabetic, is_bmp_code_point, is_decimal_digit, is_hiragana_script, is_ideographic,
    is_javanese_script, is_katakana_script, is_thai_script,
};
use crate::lcs::Range;
use crate::policy::{ComparisonPolicy, is_equal_texts, is_ws_byte};

pub fn is_punctuation(b: u32) -> bool {
    if b == 95 {
        // '_' er en del av ord
        return false;
    }
    (33..=47).contains(&b) // !"#$%&'()*+,-./
        || (58..=64).contains(&b) // :;<=>?@
        || (91..=96).contains(&b) // [\]^_`
        || (123..=126).contains(&b) // {|}~
}

pub fn is_alpha(c: char) -> bool {
    if is_white_space_code_point(c) {
        return false;
    }
    !is_punctuation(c as u32)
}

pub fn is_white_space_code_point(c: char) -> bool {
    c == '\n' || c == '\t' || c == ' '
}

/// Skrift uten mellomrom mellom ord (CJK, thai osv.). Hvert tegn blir et eget ord.
pub fn is_continuous_script(c: char) -> bool {
    let code = c as u32;
    if code < 128 {
        return false;
    }
    if is_decimal_digit(code) {
        return false;
    }

    if !is_bmp_code_point(code) {
        return true;
    }

    if is_ideographic(code) {
        return true;
    }
    if !is_alphabetic(c) {
        return true;
    }

    is_hiragana_script(code)
        || is_katakana_script(code)
        || is_thai_script(code)
        || is_javanese_script(code)
}

pub fn trim(text: &str, start: usize, end: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    trim_by(start, end, |i| is_ws_byte(bytes[i]))
}

pub fn trim_start(text: &str, start: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    trim_start_by(start, end, |i| is_ws_byte(bytes[i]))
}

pub fn trim_end(text: &str, start: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    trim_end_by(start, end, |i| is_ws_byte(bytes[i]))
}

pub fn trim_range(
    text1: &str,
    text2: &str,
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> Range {
    let (start1, end1) = trim(text1, start1, end1);
    let (start2, end2) = trim(text2, start2, end2);
    Range::new(start1, end1, start2, end2)
}

pub fn expand<T: Eq>(
    data1: &[T],
    data2: &[T],
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> Range {
    expand_by(start1, start2, end1, end2, |i1, i2| data1[i1] == data2[i2])
}

pub fn expand_forward<T: Eq>(
    data1: &[T],
    data2: &[T],
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> usize {
    expand_forward_by(start1, start2, end1, end2, |i1, i2| data1[i1] == data2[i2])
}

pub fn expand_backward<T: Eq>(
    data1: &[T],
    data2: &[T],
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> usize {
    expand_backward_by(start1, start2, end1, end2, |i1, i2| data1[i1] == data2[i2])
}

/// Skreller like whitespace i kantene av en range. Andre like tegn stopper.
pub fn expand_whitespaces(text1: &str, text2: &str, range: Range) -> Range {
    let (a, b) = (text1.as_bytes(), text2.as_bytes());
    expand_ignored_by(
        range.start1,
        range.start2,
        range.end1,
        range.end2,
        |i1, i2| a[i1] == b[i2],
        |i| is_ws_byte(a[i]),
    )
}

pub fn expand_whitespaces_forward(
    text1: &str,
    text2: &str,
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> usize {
    let (a, b) = (text1.as_bytes(), text2.as_bytes());
    expand_ignored_forward_by(
        start1,
        start2,
        end1,
        end2,
        |i1, i2| a[i1] == b[i2],
        |i| is_ws_byte(a[i]),
    )
}

pub fn expand_whitespaces_backward(
    text1: &str,
    text2: &str,
    start1: usize,
    start2: usize,
    end1: usize,
    end2: usize,
) -> usize {
    let (a, b) = (text1.as_bytes(), text2.as_bytes());
    expand_ignored_backward_by(
        start1,
        start2,
        end1,
        end2,
        |i1, i2| a[i1] == b[i2],
        |i| is_ws_byte(a[i]),
    )
}

pub fn is_equals_range(text1: &str, text2: &str, range: Range) -> bool {
    is_equal_texts(
        &text1[range.start1..range.end1],
        &text2[range.start2..range.end2],
        ComparisonPolicy::Default,
    )
}

pub fn is_equals_ignore_whitespaces_range(text1: &str, text2: &str, range: Range) -> bool {
    is_equal_texts(
        &text1[range.start1..range.end1],
        &text2[range.start2..range.end2],
        ComparisonPolicy::IgnoreWhitespaces,
    )
}

pub fn trim_by(start: usize, end: usize, mut ignored: impl FnMut(usize) -> bool) -> (usize, usize) {
    let start = trim_start_by(start, end, &mut ignored);
    let end = trim_end_by(start, end, &mut ignored);
    (start, end)
}

pub fn trim_start_by(
    mut start: usize,
    end: usize,
    mut ignored: impl FnMut(usize) -> bool,
) -> usize {
    while start < end && ignored(start) {
        start += 1;
    }
    start
}

pub fn trim_end_by(start: usize, mut end: usize, mut ignored: impl FnMut(usize) -> bool) -> usize {
    while start < end && ignored(end - 1) {
        end -= 1;
    }
    end
}

pub fn expand_by(
    mut start1: usize,
    mut start2: usize,
    mut end1: usize,
    mut end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
) -> Range {
    let count1 = expand_forward_by(start1, start2, end1, end2, &mut equals);
    start1 += count1;
    start2 += count1;

    let count2 = expand_backward_by(start1, start2, end1, end2, &mut equals);
    end1 -= count2;
    end2 -= count2;

    Range::new(start1, end1, start2, end2)
}

pub fn expand_forward_by(
    mut start1: usize,
    mut start2: usize,
    end1: usize,
    end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
) -> usize {
    let old_start1 = start1;
    while start1 < end1 && start2 < end2 && equals(start1, start2) {
        start1 += 1;
        start2 += 1;
    }
    start1 - old_start1
}

pub fn expand_backward_by(
    start1: usize,
    start2: usize,
    mut end1: usize,
    mut end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
) -> usize {
    let old_end1 = end1;
    while start1 < end1 && start2 < end2 && equals(end1 - 1, end2 - 1) {
        end1 -= 1;
        end2 -= 1;
    }
    old_end1 - end1
}

pub fn expand_ignored_by(
    mut start1: usize,
    mut start2: usize,
    mut end1: usize,
    mut end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
    mut ignored1: impl FnMut(usize) -> bool,
) -> Range {
    let count1 = expand_ignored_forward_by(start1, start2, end1, end2, &mut equals, &mut ignored1);
    start1 += count1;
    start2 += count1;

    let count2 = expand_ignored_backward_by(start1, start2, end1, end2, &mut equals, &mut ignored1);
    end1 -= count2;
    end2 -= count2;

    Range::new(start1, end1, start2, end2)
}

pub fn expand_ignored_forward_by(
    mut start1: usize,
    mut start2: usize,
    end1: usize,
    end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
    mut ignored1: impl FnMut(usize) -> bool,
) -> usize {
    let old_start1 = start1;
    while start1 < end1 && start2 < end2 && equals(start1, start2) && ignored1(start1) {
        start1 += 1;
        start2 += 1;
    }
    start1 - old_start1
}

pub fn expand_ignored_backward_by(
    start1: usize,
    start2: usize,
    mut end1: usize,
    mut end2: usize,
    mut equals: impl FnMut(usize, usize) -> bool,
    mut ignored1: impl FnMut(usize) -> bool,
) -> usize {
    let old_end1 = end1;
    while start1 < end1 && start2 < end2 && equals(end1 - 1, end2 - 1) && ignored1(end1 - 1) {
        end1 -= 1;
        end2 -= 1;
    }
    old_end1 - end1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_skreller_like_kanter() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 9, 4, 5];
        assert_eq!(expand(&a, &b, 0, 0, 5, 5), Range::new(2, 3, 2, 3));
        assert_eq!(expand(&a, &a, 0, 0, 5, 5), Range::new(5, 5, 5, 5));
        // Forover først, bakover får bare det som er igjen
        let a = [1, 1];
        let b = [1];
        assert_eq!(expand(&a, &b, 0, 0, 2, 1), Range::new(1, 2, 1, 1));
    }

    #[test]
    fn trim_whitespace() {
        assert_eq!(trim(" \tab \n", 0, 6), (2, 4));
        assert_eq!(trim("   ", 0, 3), (3, 3));
        assert_eq!(trim("x\ry", 0, 3), (0, 3));
    }

    #[test]
    fn expand_whitespaces_stopper_paa_andre_tegn() {
        // Like mellomrom skrelles, like bokstaver ikke
        let r = expand_whitespaces("  a  ", "  b  ", Range::new(0, 5, 0, 5));
        assert_eq!(r, Range::new(2, 3, 2, 3));
        let r = expand_whitespaces("xa", "xb", Range::new(0, 2, 0, 2));
        assert_eq!(r, Range::new(0, 2, 0, 2));
    }

    #[test]
    fn tegnklasser() {
        assert!(is_punctuation('.' as u32));
        assert!(!is_punctuation('_' as u32));
        assert!(!is_alpha(' '));
        assert!(is_alpha('\r'));
        assert!(!is_continuous_script('a'));
        assert!(!is_continuous_script('æ'));
        assert!(is_continuous_script('中'));
        assert!(is_continuous_script('😀'));
        assert!(is_continuous_script('→'));
        assert!(!is_continuous_script('٣'));
    }
}
