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

/// Find next match starting **after** `from` (or before if not forward).
/// `from` is a **char** index (same as `Buffer::cursor`). Returns a char index.
///
/// Wrapscan matches vim: after missing past the cursor, search the whole buffer
/// from the start (or end when going backward), so a unique match under the
/// cursor is still found. Use [`find_inclusive`] for `/` incsearch from origin.
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

/// First match **at or after** `from` (or at/before if not forward). Used by
/// incsearch so a match under the origin cursor is found while typing.
pub fn find_inclusive(rope: &Rope, pattern: &str, from: usize, forward: bool) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    let text = rope.to_string();
    let from = from.min(rope.len_chars());
    let from_byte = rope.char_to_byte(from);
    if forward {
        if text
            .get(from_byte..)
            .is_some_and(|s| s.starts_with(pattern))
        {
            return Some(from);
        }
        find(rope, pattern, from, true)
    } else {
        // Match starting at `from` counts for inclusive backward too.
        if text
            .get(from_byte..)
            .is_some_and(|s| s.starts_with(pattern))
        {
            return Some(from);
        }
        find(rope, pattern, from, false)
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

    #[test]
    fn inclusive_at_origin() {
        let rope = Rope::from_str("aa bb aa");
        assert_eq!(find_inclusive(&rope, "aa", 0, true), Some(0));
        // Exclusive next leaves the match under the cursor.
        assert_eq!(find(&rope, "aa", 0, true), Some(6));
    }
}

#[cfg(test)]
mod draft_flow {
    use crate::buffer::Buffer;
    use crate::vim::VimState;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn buffer(text: &str) -> Buffer {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        Buffer::open(file.path()).unwrap()
    }

    #[test]
    fn slash_incsearch_and_enter() {
        let mut buf = buffer("aa bb aa");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        assert!(vim.handle_char(&mut buf, "/").handled);
        assert!(vim.search_draft.is_some());
        // Type pattern: should jump to first "aa" at 0 (visible selection).
        assert!(vim.handle_char(&mut buf, "a").handled);
        assert!(vim.handle_char(&mut buf, "a").handled);
        assert_eq!(buf.cursor(), 0);
        assert!(vim.search_draft.as_ref().unwrap().has_match);
        assert!(buf.selection_range().is_some());
        // Enter commits without advancing to the next match.
        assert!(vim.handle_key(&mut buf, "enter").handled);
        assert!(vim.search_draft.is_none());
        assert_eq!(buf.cursor(), 0);
        assert_eq!(vim.search.as_ref().unwrap().pattern, "aa");
        // n → next "aa"
        assert!(vim.handle_char(&mut buf, "n").handled);
        assert_eq!(buf.cursor(), 6);
    }

    #[test]
    fn slash_escape_restores_origin() {
        let mut buf = buffer("hello world");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "w");
        assert_eq!(buf.cursor(), 6); // "world"
        vim.handle_key(&mut buf, "escape");
        assert!(vim.search_draft.is_none());
        assert_eq!(buf.cursor(), 0);
        assert!(buf.selection_range().is_none());
        // Esc does not commit the search.
        assert!(vim.search.is_none());
    }

    #[test]
    fn empty_slash_reuses_last() {
        let mut buf = buffer("one two one");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "t");
        vim.handle_char(&mut buf, "w");
        vim.handle_char(&mut buf, "o");
        vim.handle_key(&mut buf, "enter");
        assert_eq!(buf.cursor(), 4);
        // Move back, empty / reuses "two"
        buf.set_cursor_raw(0);
        buf.clear_selection();
        vim.handle_char(&mut buf, "/");
        vim.handle_key(&mut buf, "enter");
        assert_eq!(buf.cursor(), 4);
    }

    #[test]
    fn no_match_status() {
        let mut buf = buffer("hello");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "z");
        assert!(!vim.search_draft.as_ref().unwrap().has_match);
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn control_chars_ignored_in_pattern() {
        let mut buf = buffer("ab");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "a");
        vim.handle_char(&mut buf, "\n");
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "a");
    }

    #[test]
    fn search_key_does_not_claim_printable() {
        // Regression: claiming handled for "f" blocked macOS insertText / key_char.
        let mut buf = buffer("foo");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        let r = vim.handle_key(&mut buf, "f");
        assert!(
            !r.handled,
            "printable keys must not be handled by search key path"
        );
        // Char path still appends.
        assert!(vim.handle_char(&mut buf, "f").handled);
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "f");
    }

    #[test]
    fn multi_char_append_builds_pattern() {
        let mut buf = buffer("hello");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        assert!(vim.handle_char(&mut buf, "el").handled);
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "el");
        assert!(vim.search_draft.as_ref().unwrap().has_match);
    }
}
