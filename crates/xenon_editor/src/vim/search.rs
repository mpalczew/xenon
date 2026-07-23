//! Buffer search for `/`, `?`, `n`, `N`, `*`.
//!
//! Smartcase (good-enough vim): case-insensitive when the pattern has no
//! uppercase letters; case-sensitive when it does. Matches the common
//! `ignorecase` + `smartcase` pair without a settings surface.

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
    search(rope, pattern, from, forward, false)
}

/// First match **at or after** `from` (or at/before if not forward). Used by
/// incsearch so a match under the origin cursor is found while typing.
pub fn find_inclusive(rope: &Rope, pattern: &str, from: usize, forward: bool) -> Option<usize> {
    search(rope, pattern, from, forward, true)
}

fn search(
    rope: &Rope,
    pattern: &str,
    from: usize,
    forward: bool,
    inclusive: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    let text = rope.to_string();
    let from = from.min(rope.len_chars());
    let from_byte = rope.char_to_byte(from);
    let case_sensitive = pattern.chars().any(|c| c.is_uppercase());
    if inclusive {
        let hay = &text[from_byte..];
        let at = if case_sensitive {
            hay.starts_with(pattern)
        } else {
            let mut hc = hay.chars();
            pattern.chars().all(|pc| {
                hc.next()
                    .is_some_and(|c| c == pc || c.to_lowercase().eq(pc.to_lowercase()))
            })
        };
        if at {
            return Some(from);
        }
    }
    let byte = if case_sensitive {
        find_exact(&text, pattern, from_byte, forward)?
    } else {
        find_ci(&text, pattern, from_byte, forward)?
    };
    Some(rope.byte_to_char(byte))
}

fn find_exact(text: &str, pattern: &str, from_byte: usize, forward: bool) -> Option<usize> {
    if forward {
        let start = next_byte_boundary(text, from_byte);
        if let Some(rel) = text.get(start..)?.find(pattern) {
            return Some(start + rel);
        }
        text.find(pattern)
    } else if let Some(byte) = text.get(..from_byte)?.rfind(pattern) {
        Some(byte)
    } else {
        text.rfind(pattern)
    }
}

/// Case-insensitive via lowercased haystack. Best-effort for non-ASCII casefold
/// (same spirit as cmd-f).
fn find_ci(text: &str, pattern: &str, from_byte: usize, forward: bool) -> Option<usize> {
    let needle = pattern.to_lowercase();
    if needle.is_empty() {
        return None;
    }
    let hay = text.to_lowercase();
    let stable = hay.len() == text.len();
    let to_hay = |text_byte: usize| -> usize {
        if stable {
            text_byte
        } else {
            hay.chars()
                .take(text[..text_byte].chars().count())
                .map(|c| c.len_utf8())
                .sum()
        }
    };
    let from_hay = |hay_byte: usize| -> usize {
        if stable {
            hay_byte
        } else {
            text.chars()
                .take(hay[..hay_byte].chars().count())
                .map(|c| c.len_utf8())
                .sum()
        }
    };
    if forward {
        let start = to_hay(next_byte_boundary(text, from_byte));
        if let Some(rel) = hay.get(start..)?.find(&needle) {
            return Some(from_hay(start + rel));
        }
        hay.find(&needle).map(from_hay)
    } else {
        let end = to_hay(from_byte.min(text.len()));
        if let Some(b) = hay.get(..end)?.rfind(&needle) {
            return Some(from_hay(b));
        }
        hay.rfind(&needle).map(from_hay)
    }
}

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
    fn core_find() {
        let rope = Rope::from_str("ab x ab");
        assert_eq!(find(&rope, "ab", 0, true), Some(5));
        assert_eq!(find(&rope, "", 0, true), None);
        assert_eq!(find(&rope, "ab", 6, true), Some(0));
        assert_eq!(find(&rope, "ab", 6, false), Some(0));
        assert_eq!(find(&rope, "ab", 0, false), Some(5));

        let rope = Rope::from_str("only once here");
        assert_eq!(find(&rope, "once", 5, true), Some(5));

        let rope = Rope::from_str("a日b hello");
        let from = rope.to_string().chars().count() - 1;
        let pos = find(&rope, "hello", 0, true).unwrap();
        assert_eq!(rope.to_string().chars().nth(pos), Some('h'));
        assert_eq!(find(&rope, "hello", from, true), Some(pos));

        let rope = Rope::from_str("aa bb aa");
        assert_eq!(find_inclusive(&rope, "aa", 0, true), Some(0));
        assert_eq!(find(&rope, "aa", 0, true), Some(6));
    }

    #[test]
    fn smartcase() {
        let rope = Rope::from_str("Foo foo FOO");
        assert_eq!(find_inclusive(&rope, "foo", 0, true), Some(0));
        assert_eq!(find(&rope, "foo", 0, true), Some(4));
        assert_eq!(find(&rope, "foo", 4, true), Some(8));
        // Exclusive backward from inside third match lands on second.
        assert_eq!(find(&rope, "foo", 9, false), Some(4));
        assert_eq!(find_inclusive(&rope, "Foo", 0, true), Some(0));
        assert_eq!(find(&rope, "Foo", 1, true), Some(0));
        assert_eq!(find_inclusive(&rope, "FOO", 0, true), Some(8));
        assert_eq!(find(&rope, "FoO", 0, true), None);
    }

    #[test]
    fn slash_incsearch_and_enter() {
        let mut buf = buffer("aa bb aa");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        assert!(vim.handle_char(&mut buf, "/").handled);
        assert!(vim.search_draft.is_some());
        assert!(vim.handle_char(&mut buf, "a").handled);
        assert!(vim.handle_char(&mut buf, "a").handled);
        assert_eq!(buf.cursor(), 0);
        assert!(vim.search_draft.as_ref().unwrap().has_match);
        assert!(buf.selection_range().is_some());
        assert!(vim.handle_key(&mut buf, "enter").handled);
        assert!(vim.search_draft.is_none());
        assert_eq!(buf.cursor(), 0);
        assert_eq!(vim.search.as_ref().unwrap().pattern, "aa");
        assert!(vim.handle_char(&mut buf, "n").handled);
        assert_eq!(buf.cursor(), 6);
    }

    #[test]
    fn slash_escape_and_empty_reuse() {
        let mut buf = buffer("hello world");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "w");
        assert_eq!(buf.cursor(), 6);
        vim.handle_key(&mut buf, "escape");
        assert!(vim.search_draft.is_none());
        assert_eq!(buf.cursor(), 0);
        assert!(buf.selection_range().is_none());
        assert!(vim.search.is_none());

        let mut buf = buffer("one two one");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        for c in ['t', 'w', 'o'] {
            vim.handle_char(&mut buf, &c.to_string());
        }
        vim.handle_key(&mut buf, "enter");
        assert_eq!(buf.cursor(), 4);
        buf.set_cursor_raw(0);
        buf.clear_selection();
        vim.handle_char(&mut buf, "/");
        vim.handle_key(&mut buf, "enter");
        assert_eq!(buf.cursor(), 4);
    }

    #[test]
    fn draft_edge_cases() {
        let mut buf = buffer("hello");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "z");
        assert!(!vim.search_draft.as_ref().unwrap().has_match);
        assert_eq!(buf.cursor(), 0);

        let mut buf = buffer("ab");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        vim.handle_char(&mut buf, "a");
        vim.handle_char(&mut buf, "\n");
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "a");

        let mut buf = buffer("foo");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        assert!(
            !vim.handle_key(&mut buf, "f").handled,
            "printable keys must not be handled by search key path"
        );
        assert!(vim.handle_char(&mut buf, "f").handled);
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "f");

        let mut buf = buffer("hello");
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        assert!(vim.handle_char(&mut buf, "el").handled);
        assert_eq!(vim.search_draft.as_ref().unwrap().pattern, "el");
        assert!(vim.search_draft.as_ref().unwrap().has_match);
    }

    #[test]
    fn slash_smartcase_flow() {
        let mut buf = buffer("error Error ERROR");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        for c in ['e', 'r', 'r', 'o', 'r'] {
            assert!(vim.handle_char(&mut buf, &c.to_string()).handled);
        }
        assert_eq!(buf.cursor(), 0);
        vim.handle_key(&mut buf, "enter");
        assert!(vim.handle_char(&mut buf, "n").handled);
        assert_eq!(buf.cursor(), 6);
        assert!(vim.handle_char(&mut buf, "n").handled);
        assert_eq!(buf.cursor(), 12);

        let mut buf = buffer("error Error ERROR");
        buf.set_cursor_raw(0);
        let mut vim = VimState::default();
        vim.handle_char(&mut buf, "/");
        for c in ['E', 'r', 'r', 'o', 'r'] {
            assert!(vim.handle_char(&mut buf, &c.to_string()).handled);
        }
        assert_eq!(buf.cursor(), 6);
        vim.handle_key(&mut buf, "enter");
        assert!(vim.handle_char(&mut buf, "n").handled);
        assert_eq!(buf.cursor(), 6);
    }
}
