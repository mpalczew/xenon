//! Buffer search for `/`, `?`, `n`, `N`, `*`.

use ropey::Rope;

#[derive(Clone, Debug)]
pub struct SearchState {
    pub pattern: String,
    pub forward: bool,
}

impl SearchState {
    pub fn new(pattern: String, forward: bool) -> Self {
        Self { pattern, forward }
    }
}

/// Find next match starting after `from` (or before if not forward).
/// `from` is a **char** index (same as `Buffer::cursor`). Returns a char index.
///
/// Wrapscan matches vim: after missing past the cursor, search the whole buffer
/// from the start (or end when going backward), so a unique match under the
/// cursor is still found.
pub fn find(rope: &Rope, pattern: &str, from: usize, forward: bool) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    let text = rope.to_string();
    let from = from.min(rope.len_chars());
    let from_byte = rope.char_to_byte(from);
    if forward {
        let start = next_byte_boundary(&text, from_byte);
        if let Some(rel) = text.get(start..)?.find(pattern) {
            return Some(rope.byte_to_char(start + rel));
        }
        // Wrap: full buffer from the start.
        text.find(pattern).map(|byte| rope.byte_to_char(byte))
    } else {
        if let Some(byte) = text.get(..from_byte)?.rfind(pattern) {
            return Some(rope.byte_to_char(byte));
        }
        // Wrap: full buffer from the end.
        text.rfind(pattern).map(|byte| rope.byte_to_char(byte))
    }
}

/// First byte strictly after `from_byte` that is a char boundary (or `len`).
fn next_byte_boundary(text: &str, from_byte: usize) -> usize {
    let len = text.len();
    if from_byte >= len {
        return len;
    }
    let mut i = from_byte + 1;
    while i < len && !text.is_char_boundary(i) {
        i += 1;
    }
    i.min(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_next() {
        let rope = Rope::from_str("ab x ab");
        assert_eq!(find(&rope, "ab", 0, true), Some(5));
    }

    #[test]
    fn empty_pattern() {
        let rope = Rope::from_str("hello");
        assert_eq!(find(&rope, "", 0, true), None);
    }

    #[test]
    fn wrap_unique_match_under_cursor() {
        let rope = Rope::from_str("only once here");
        // "once" starts at char 5; cursor on that match.
        assert_eq!(find(&rope, "once", 5, true), Some(5));
    }

    #[test]
    fn wrap_finds_earlier_match() {
        let rope = Rope::from_str("ab x ab");
        // Cursor past second "ab"; wrap to first.
        assert_eq!(find(&rope, "ab", 6, true), Some(0));
    }

    #[test]
    fn unicode_before_cursor() {
        let rope = Rope::from_str("a日b hello");
        // "hello" starts after "a日b " — must not panic on multi-byte 日.
        let from = rope.to_string().chars().count() - 1; // last char
        let pos = find(&rope, "hello", 0, true).unwrap();
        assert_eq!(rope.to_string().chars().nth(pos), Some('h'));
        // Search from end still finds via wrap.
        assert_eq!(find(&rope, "hello", from, true), Some(pos));
    }

    #[test]
    fn backward_find() {
        let rope = Rope::from_str("ab x ab");
        assert_eq!(find(&rope, "ab", 6, false), Some(0));
    }

    #[test]
    fn backward_wrap() {
        let rope = Rope::from_str("ab x ab");
        // Cursor before first match; wrap to last.
        assert_eq!(find(&rope, "ab", 0, false), Some(5));
    }
}
