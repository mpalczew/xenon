//! Match-pair (`%`) and file-percent (`N%`) motions.

use ropey::Rope;

/// Bracket pairs for `%` (classic vim, without matchit).
const PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

fn pair_kind(ch: char) -> Option<(char, char, bool)> {
    for &(open, close) in PAIRS {
        if ch == open {
            return Some((open, close, true));
        }
        if ch == close {
            return Some((open, close, false));
        }
    }
    None
}

/// Find next `()`/`[]`/`{}` under or after cursor on this line, then its match.
pub fn match_pair(rope: &Rope, cursor: usize) -> usize {
    let len = rope.len_chars();
    if len == 0 {
        return 0;
    }
    let start = cursor.min(len - 1);
    let mut item = None;
    let mut i = start;
    while i < len {
        let ch = rope.char(i);
        if ch == '\n' {
            break;
        }
        if let Some(kind) = pair_kind(ch) {
            item = Some((i, kind));
            break;
        }
        i += 1;
    }
    let Some((pos, (open, close, is_open))) = item else {
        return cursor;
    };
    if is_open {
        match_forward(rope, pos, open, close).unwrap_or(cursor)
    } else {
        match_backward(rope, pos, open, close).unwrap_or(cursor)
    }
}

fn match_forward(rope: &Rope, open_at: usize, open: char, close: char) -> Option<usize> {
    let len = rope.len_chars();
    let mut depth = 0usize;
    let mut j = open_at + 1;
    while j < len {
        let ch = rope.char(j);
        if ch == open {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                return Some(j);
            }
            depth -= 1;
        }
        j += 1;
    }
    None
}

fn match_backward(rope: &Rope, close_at: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut j = close_at;
    while j > 0 {
        j -= 1;
        let ch = rope.char(j);
        if ch == close {
            depth += 1;
        } else if ch == open {
            if depth == 0 {
                return Some(j);
            }
            depth -= 1;
        }
    }
    None
}

/// `{pct}%` of file: line `pct * line_count / 100` (1-based, clamped).
pub fn percent_of_file(rope: &Rope, pct: usize) -> usize {
    let n_lines = rope.len_lines().max(1);
    let pct = pct.min(100);
    let line = ((pct * n_lines) / 100).max(1).min(n_lines);
    let row = line - 1;
    first_non_blank(rope, rope.line_to_char(row))
}

fn first_non_blank(rope: &Rope, at: usize) -> usize {
    if rope.len_chars() == 0 {
        return 0;
    }
    let row = rope.char_to_line(at.min(rope.len_chars().saturating_sub(1)));
    let start = rope.line_to_char(row);
    let line = rope.line(row);
    let mut content_len = line.len_chars();
    if content_len > 0 && line.char(content_len - 1) == '\n' {
        content_len -= 1;
    }
    let end = start + content_len;
    let mut i = start;
    while i < end {
        if !rope.char(i).is_whitespace() {
            return i;
        }
        i += 1;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_pair_open_to_close() {
        let rope = Rope::from_str("a(bc)d");
        assert_eq!(match_pair(&rope, 1), 4);
    }

    #[test]
    fn match_pair_close_to_open() {
        let rope = Rope::from_str("a(bc)d");
        assert_eq!(match_pair(&rope, 4), 1);
    }

    #[test]
    fn match_pair_finds_next_on_line() {
        let rope = Rope::from_str("x (ab)");
        assert_eq!(match_pair(&rope, 0), 5);
    }

    #[test]
    fn match_pair_nested() {
        let rope = Rope::from_str("(a(b)c)");
        assert_eq!(match_pair(&rope, 0), 6);
        assert_eq!(match_pair(&rope, 2), 4);
    }

    #[test]
    fn match_pair_braces_and_brackets() {
        let rope = Rope::from_str("{[x]}");
        assert_eq!(match_pair(&rope, 0), 4);
        assert_eq!(match_pair(&rope, 1), 3);
    }

    #[test]
    fn percent_of_file_mid() {
        let rope = Rope::from_str("a\nb\nc\nd\n");
        let pos = percent_of_file(&rope, 50);
        let row = rope.char_to_line(pos.min(rope.len_chars().saturating_sub(1)));
        assert!(row > 0);
    }
}
