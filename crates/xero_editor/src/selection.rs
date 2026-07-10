//! Selection helpers: ranges, word/line bounds. Pure (no gpui).

use std::ops::Range;

use ropey::Rope;

/// Half-open char range for a non-empty selection, or `None` if empty.
pub fn range(anchor: Option<usize>, cursor: usize) -> Option<Range<usize>> {
    let anchor = anchor?;
    if anchor == cursor {
        return None;
    }
    let (start, end) = if anchor < cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };
    Some(start..end)
}

/// Word char range containing `offset` (or adjacent if on a boundary).
/// Word chars: alphanumeric or `_`. Otherwise a run of non-word non-space, or a single space.
pub fn word_range_at(rope: &Rope, offset: usize) -> Range<usize> {
    let len = rope.len_chars();
    if len == 0 {
        return 0..0;
    }
    let offset = offset.min(len.saturating_sub(1));
    let ch = rope.char(offset);
    let class = char_class(ch);
    let mut start = offset;
    while start > 0 && char_class(rope.char(start - 1)) == class {
        start -= 1;
    }
    let mut end = offset + 1;
    while end < len && char_class(rope.char(end)) == class {
        end += 1;
    }
    start..end
}

/// Line content range containing `offset`, excluding the trailing newline.
pub fn line_range_at(rope: &Rope, offset: usize) -> Range<usize> {
    let len = rope.len_chars();
    if len == 0 {
        return 0..0;
    }
    let offset = offset.min(len);
    let row = if offset == len {
        rope.len_lines().saturating_sub(1)
    } else {
        rope.char_to_line(offset)
    };
    let start = rope.line_to_char(row);
    let line = rope.line(row);
    let line_chars = line.len_chars();
    let content = if line_chars > 0 && line.char(line_chars - 1) == '\n' {
        line_chars - 1
    } else {
        line_chars
    };
    start..start + content
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn char_class(ch: char) -> CharClass {
    if ch.is_alphanumeric() || ch == '_' {
        CharClass::Word
    } else if ch.is_whitespace() {
        CharClass::Space
    } else {
        CharClass::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn empty_selection_when_equal() {
        assert!(range(Some(3), 3).is_none());
        assert!(range(None, 3).is_none());
    }

    #[test]
    fn ordered_range() {
        assert_eq!(range(Some(5), 2), Some(2..5));
        assert_eq!(range(Some(2), 5), Some(2..5));
    }

    #[test]
    fn word_range_identifier() {
        let rope = Rope::from_str("foo bar_baz");
        assert_eq!(word_range_at(&rope, 1), 0..3);
        assert_eq!(word_range_at(&rope, 5), 4..11);
    }

    #[test]
    fn line_range_excludes_newline() {
        let rope = Rope::from_str("ab\ncd\n");
        assert_eq!(line_range_at(&rope, 1), 0..2);
        assert_eq!(line_range_at(&rope, 3), 3..5);
    }
}
