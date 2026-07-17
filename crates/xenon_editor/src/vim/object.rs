//! Text objects: iw/aw and simple quote/bracket pairs.

use std::ops::Range;

use ropey::Rope;

use crate::selection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    Word {
        around: bool,
    },
    Quote {
        ch: char,
        around: bool,
    },
    Pair {
        open: char,
        close: char,
        around: bool,
    },
}

impl Object {
    pub fn range(self, rope: &Rope, cursor: usize) -> Option<Range<usize>> {
        match self {
            Object::Word { around } => {
                let inner =
                    selection::word_range_at(rope, cursor.min(rope.len_chars().saturating_sub(1)));
                if !around {
                    return Some(inner);
                }
                Some(around_word(rope, inner))
            }
            Object::Quote { ch, around } => find_pair(rope, cursor, ch, ch, around),
            Object::Pair {
                open,
                close,
                around,
            } => find_pair(rope, cursor, open, close, around),
        }
    }
}

fn around_word(rope: &Rope, inner: Range<usize>) -> Range<usize> {
    let mut start = inner.start;
    let mut end = inner.end;
    let len = rope.len_chars();
    while end < len && rope.char(end).is_whitespace() && rope.char(end) != '\n' {
        end += 1;
    }
    if end == inner.end {
        while start > 0 && rope.char(start - 1).is_whitespace() && rope.char(start - 1) != '\n' {
            start -= 1;
        }
    }
    start..end
}

fn find_pair(
    rope: &Rope,
    cursor: usize,
    open: char,
    close: char,
    around: bool,
) -> Option<Range<usize>> {
    let len = rope.len_chars();
    if len == 0 {
        return None;
    }
    let cursor = cursor.min(len - 1);
    // Search backward for open.
    let mut depth = 0usize;
    let mut open_at = None;
    let mut i = cursor + 1;
    while i > 0 {
        i -= 1;
        let ch = rope.char(i);
        if ch == close && open != close {
            depth += 1;
        } else if ch == open {
            if depth == 0 {
                open_at = Some(i);
                break;
            }
            depth = depth.saturating_sub(1);
        } else if open == close && ch == open {
            open_at = Some(i);
            break;
        }
    }
    let open_at = open_at?;
    // Search forward for close.
    depth = 0;
    let mut j = open_at + 1;
    let mut close_at = None;
    while j < len {
        let ch = rope.char(j);
        if ch == open && open != close {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                close_at = Some(j);
                break;
            }
            depth = depth.saturating_sub(1);
        }
        j += 1;
    }
    let close_at = close_at?;
    if around {
        Some(open_at..close_at + 1)
    } else {
        Some(open_at + 1..close_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_word() {
        let rope = Rope::from_str("foo bar");
        let r = Object::Word { around: false }.range(&rope, 1).unwrap();
        assert_eq!(rope.slice(r).to_string(), "foo");
    }

    #[test]
    fn inner_parens() {
        let rope = Rope::from_str("a(bc)d");
        let r = Object::Pair {
            open: '(',
            close: ')',
            around: false,
        }
        .range(&rope, 2)
        .unwrap();
        assert_eq!(rope.slice(r).to_string(), "bc");
    }
}
