//! Motions over a rope, returning a target cursor index.

use ropey::Rope;

use super::pair;
use crate::selection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    FirstNonBlank,
    WordForward,
    WordEnd,
    WordBackward,
    WORDForward,
    WORDBackward,
    FileStart,
    FileEnd,
    Find {
        ch: char,
        before: bool,
        forward: bool,
    },
    /// Jump to matching `()`/`[]`/`{}` (bare `%`).
    MatchPair,
    /// Go to `pct` percent of the file (`50%`). Linewise.
    Percent {
        pct: usize,
    },
    /// Current line, `count` lines down (`_`, and the motion in `dd`/`yy`).
    /// Count 1 stays on this line; count 2 is one line down.
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionKind {
    Exclusive,
    Inclusive,
}

impl Motion {
    pub fn kind(&self) -> MotionKind {
        match self {
            Motion::Left
            | Motion::Right
            | Motion::Up
            | Motion::Down
            | Motion::LineStart
            | Motion::FirstNonBlank
            | Motion::WordForward
            | Motion::WordBackward
            | Motion::WORDForward
            | Motion::WORDBackward
            | Motion::FileStart
            | Motion::FileEnd
            | Motion::Percent { .. }
            | Motion::Line => MotionKind::Exclusive,
            Motion::LineEnd
            | Motion::WordEnd
            | Motion::MatchPair
            | Motion::Find { before: false, .. } => MotionKind::Inclusive,
            Motion::Find { before: true, .. } => MotionKind::Exclusive,
        }
    }

    pub fn is_linewise(&self) -> bool {
        matches!(
            self,
            Motion::Up
                | Motion::Down
                | Motion::FileStart
                | Motion::FileEnd
                | Motion::Percent { .. }
                | Motion::Line
        )
    }
}

/// Apply `count` repetitions of `motion` from `cursor`.
pub fn apply(rope: &Rope, cursor: usize, motion: &Motion, count: usize) -> usize {
    let count = count.max(1);
    if matches!(motion, Motion::Line) {
        // `_` / `dd`: count 1 is this line; extra count walks down.
        let mut pos = cursor;
        for _ in 1..count {
            pos = vertical(rope, pos, 1);
        }
        return first_non_blank(rope, pos);
    }
    let mut pos = cursor;
    for _ in 0..count {
        pos = step(rope, pos, motion);
    }
    pos.min(rope.len_chars())
}

fn step(rope: &Rope, cursor: usize, motion: &Motion) -> usize {
    let len = rope.len_chars();
    match motion {
        // Classic vim: h/l do not wrap across lines (no whichwrap).
        Motion::Left => {
            if len == 0 {
                0
            } else {
                let start = line_start(rope, cursor);
                if cursor > start { cursor - 1 } else { cursor }
            }
        }
        Motion::Right => {
            if len == 0 {
                0
            } else {
                // Last content char (never the newline); `l` does not wrap.
                let max = line_end(rope, cursor);
                if cursor >= max { cursor } else { cursor + 1 }
            }
        }
        Motion::Up => vertical(rope, cursor, -1),
        Motion::Down => vertical(rope, cursor, 1),
        Motion::LineStart => line_start(rope, cursor),
        Motion::LineEnd => line_end(rope, cursor),
        Motion::FirstNonBlank => first_non_blank(rope, cursor),
        Motion::WordForward => word_forward(rope, cursor, false),
        Motion::WORDForward => word_forward(rope, cursor, true),
        Motion::WordEnd => word_end(rope, cursor),
        Motion::WordBackward => word_backward(rope, cursor, false),
        Motion::WORDBackward => word_backward(rope, cursor, true),
        Motion::FileStart => 0,
        Motion::FileEnd => {
            if len == 0 {
                0
            } else {
                line_start(rope, len.saturating_sub(1))
            }
        }
        Motion::Find {
            ch,
            before,
            forward,
        } => find_char(rope, cursor, *ch, *before, *forward),
        Motion::MatchPair => pair::match_pair(rope, cursor),
        Motion::Percent { pct } => pair::percent_of_file(rope, *pct),
        Motion::Line => first_non_blank(rope, cursor),
    }
}

fn vertical(rope: &Rope, cursor: usize, delta: isize) -> usize {
    if rope.len_chars() == 0 {
        return 0;
    }
    let row = rope.char_to_line(cursor.min(rope.len_chars().saturating_sub(1)));
    let col = cursor - rope.line_to_char(row);
    let target = row as isize + delta;
    if target < 0 || target as usize >= rope.len_lines() {
        return cursor;
    }
    let target = target as usize;
    let line_len = line_content_len(rope, target);
    rope.line_to_char(target) + col.min(line_len)
}

fn line_start(rope: &Rope, cursor: usize) -> usize {
    if rope.len_chars() == 0 {
        return 0;
    }
    let row = rope.char_to_line(cursor.min(rope.len_chars().saturating_sub(1)));
    rope.line_to_char(row)
}

/// Index of the last content character on the line (`$` / normal-mode EOL).
/// Empty line → line start (often the newline char itself). Never past content.
fn line_end(rope: &Rope, cursor: usize) -> usize {
    let start = line_start(rope, cursor);
    let exclusive = line_end_exclusive(rope, cursor);
    if exclusive > start {
        exclusive - 1
    } else {
        start
    }
}

/// Index just after the line's content (newline, or `len` if no trailing newline).
/// Used for insert-at-EOL (`A`, `o`) and as the exclusive end for `d$`.
pub(in crate::vim) fn line_end_exclusive(rope: &Rope, cursor: usize) -> usize {
    let start = line_start(rope, cursor);
    if rope.len_chars() == 0 {
        return 0;
    }
    let row = rope.char_to_line(cursor.min(rope.len_chars().saturating_sub(1)));
    start + line_content_len(rope, row)
}

fn first_non_blank(rope: &Rope, cursor: usize) -> usize {
    let start = line_start(rope, cursor);
    let end = line_end_exclusive(rope, cursor);
    let mut i = start;
    while i < end {
        let ch = rope.char(i);
        if !ch.is_whitespace() {
            return i;
        }
        i += 1;
    }
    start
}

fn line_content_len(rope: &Rope, row: usize) -> usize {
    let line = rope.line(row);
    let len = line.len_chars();
    if len > 0 && line.char(len - 1) == '\n' {
        len - 1
    } else {
        len
    }
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn word_forward(rope: &Rope, cursor: usize, big: bool) -> usize {
    let len = rope.len_chars();
    if cursor >= len {
        return len;
    }
    let mut i = cursor;
    if big {
        while i < len && !rope.char(i).is_whitespace() {
            i += 1;
        }
    } else if is_word(rope.char(i)) {
        while i < len && is_word(rope.char(i)) {
            i += 1;
        }
    } else if !rope.char(i).is_whitespace() {
        while i < len && !is_word(rope.char(i)) && !rope.char(i).is_whitespace() {
            i += 1;
        }
    }
    while i < len && rope.char(i).is_whitespace() {
        i += 1;
    }
    i
}

fn word_end(rope: &Rope, cursor: usize) -> usize {
    let len = rope.len_chars();
    if len == 0 {
        return 0;
    }
    let mut i = (cursor + 1).min(len - 1);
    while i < len && rope.char(i).is_whitespace() {
        i += 1;
    }
    if i >= len {
        return len.saturating_sub(1);
    }
    if is_word(rope.char(i)) {
        while i + 1 < len && is_word(rope.char(i + 1)) {
            i += 1;
        }
    } else {
        while i + 1 < len && !is_word(rope.char(i + 1)) && !rope.char(i + 1).is_whitespace() {
            i += 1;
        }
    }
    i
}

fn word_backward(rope: &Rope, cursor: usize, big: bool) -> usize {
    if cursor == 0 {
        return 0;
    }
    let mut i = cursor - 1;
    while i > 0 && rope.char(i).is_whitespace() {
        i -= 1;
    }
    if big {
        while i > 0 && !rope.char(i - 1).is_whitespace() {
            i -= 1;
        }
    } else if is_word(rope.char(i)) {
        while i > 0 && is_word(rope.char(i - 1)) {
            i -= 1;
        }
    } else {
        while i > 0 && !is_word(rope.char(i - 1)) && !rope.char(i - 1).is_whitespace() {
            i -= 1;
        }
    }
    i
}

fn find_char(rope: &Rope, cursor: usize, ch: char, before: bool, forward: bool) -> usize {
    let len = rope.len_chars();
    if forward {
        let mut i = cursor + 1;
        while i < len {
            if rope.char(i) == ch {
                return if before { i.saturating_sub(1) } else { i };
            }
            if rope.char(i) == '\n' {
                break;
            }
            i += 1;
        }
        cursor
    } else {
        if cursor == 0 {
            return 0;
        }
        let mut i = cursor - 1;
        loop {
            if rope.char(i) == ch {
                return if before { (i + 1).min(len) } else { i };
            }
            if rope.char(i) == '\n' || i == 0 {
                break;
            }
            i -= 1;
        }
        cursor
    }
}

/// Inclusive range from cursor to motion target for operators.
pub fn operator_range(
    rope: &Rope,
    cursor: usize,
    motion: &Motion,
    count: usize,
) -> std::ops::Range<usize> {
    // `$` / `D`: through last content char only — never the newline (no line join).
    if matches!(motion, Motion::LineEnd) {
        let end = line_end_exclusive(rope, cursor);
        let start = cursor.min(end);
        return start..end;
    }
    let target = apply(rope, cursor, motion, count);
    if motion.is_linewise() {
        let a = selection::line_range_at(rope, cursor);
        let b = selection::line_range_at(rope, target);
        let start = a.start.min(b.start);
        let end_line = selection::line_range_at(rope, target.max(cursor));
        // Include newline after the last line when possible.
        let mut end = end_line.end.max(a.end);
        if end < rope.len_chars() && rope.char(end) == '\n' {
            end += 1;
        }
        return start..end;
    }
    let (start, mut end) = if target >= cursor {
        (cursor, target)
    } else {
        (target, cursor)
    };
    if matches!(motion.kind(), MotionKind::Inclusive) && end < rope.len_chars() {
        // Inclusive endpoint is a character index; include it, but never a newline.
        if rope.char(end) != '\n' {
            end = (end + 1).min(rope.len_chars());
        }
    }
    if start == end && end < rope.len_chars() && rope.char(end) != '\n' {
        end += 1;
    }
    start..end
}
