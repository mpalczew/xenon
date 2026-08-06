//! Join, indent, and number-bump edits for vim mode.

use super::repeat::LastChange;
use super::{HandleResult, Mode, Operator, VimState, edited, handled};
use crate::buffer::Buffer;
use crate::selection;
use crate::undo::Edit;

impl VimState {
    pub(in crate::vim) fn join_lines(
        &mut self,
        buffer: &mut Buffer,
        lines: usize,
        space: bool,
    ) -> HandleResult {
        let lines = lines.max(2);
        self.last_change = Some(LastChange::Join { lines, space });
        let mut any = false;
        let mut cursor_on_join = None;
        for _ in 0..(lines - 1) {
            let Some(pos) = join_once(buffer, space) else {
                break;
            };
            cursor_on_join = Some(pos);
            any = true;
        }
        if let Some(pos) = cursor_on_join {
            buffer.set_cursor_raw(pos);
        }
        buffer.clear_selection();
        self.count = 0;
        self.mode = Mode::Normal;
        if any { edited() } else { handled(false) }
    }

    /// Visual `J` / `gJ`: join every line in the selection.
    pub(in crate::vim) fn join_visual(&mut self, buffer: &mut Buffer, space: bool) -> HandleResult {
        let (start_line, end_line) = if self.mode == Mode::VisualBlock {
            let Some(corners) = self.block_corners(buffer) else {
                self.mode = Mode::Normal;
                self.block_anchor = None;
                return handled(false);
            };
            (corners.min_row, corners.max_row)
        } else {
            let Some(range) = buffer.selection_range() else {
                self.mode = Mode::Normal;
                return handled(false);
            };
            let start_line = buffer.rope().char_to_line(range.start);
            let end_idx = range
                .end
                .saturating_sub(1)
                .min(buffer.rope().len_chars().saturating_sub(1));
            let end_line = buffer.rope().char_to_line(end_idx);
            (start_line, end_line)
        };
        let n = end_line.saturating_sub(start_line) + 1;
        if n < 2 {
            buffer.clear_selection();
            self.block_anchor = None;
            self.mode = Mode::Normal;
            return handled(false);
        }
        // Cursor at first selected line, then join.
        let line_start = buffer.rope().line_to_char(start_line);
        buffer.set_cursor_raw(line_start);
        buffer.clear_selection();
        self.block_anchor = None;
        self.mode = Mode::Normal;
        self.join_lines(buffer, n, space)
    }

    /// `Ctrl-a` / `Ctrl-x`: add `delta * count` to the number at/after the cursor.
    pub(crate) fn change_number(
        &mut self,
        buffer: &mut Buffer,
        delta: i64,
        count: usize,
    ) -> HandleResult {
        let step = delta.saturating_mul(count.max(1) as i64);
        // Store full step so `.` repeats the same bump (including count).
        self.last_change = Some(LastChange::Number { delta: step });
        self.count = 0;
        let Some((range, value)) = find_number(buffer.rope(), buffer.cursor()) else {
            return handled(false);
        };
        let new_val = value.saturating_add(step);
        let new = new_val.to_string();
        let old = buffer.rope().slice(range.clone()).to_string();
        let new = preserve_width(&old, &new);
        buffer.apply_edit(Edit {
            start: range.start,
            old,
            new: new.clone(),
        });
        // Cursor on last digit of the new number (vim).
        buffer.set_cursor_raw(range.start + new.chars().count().saturating_sub(1));
        buffer.clear_selection();
        edited()
    }

    pub(in crate::vim) fn shift_range(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        range: std::ops::Range<usize>,
    ) -> HandleResult {
        let rope = buffer.rope();
        let len = rope.len_chars();
        if len == 0 {
            self.clear_pending();
            return handled(false);
        }
        let start = range.start.min(len);
        let end = range.end.min(len).max(start);
        let first = rope.char_to_line(start);
        // Exclusive end: last line is the one containing end-1 when range is non-empty.
        let last = if end > start {
            rope.char_to_line(end.saturating_sub(1)).max(first)
        } else {
            first
        };
        let prev_indent = if first > 0 {
            line_indent(rope, first - 1)
        } else {
            String::new()
        };
        // Bottom-up so earlier offsets stay valid.
        let mut any = false;
        for row in (first..=last).rev() {
            if shift_line(buffer, row, op, &prev_indent) {
                any = true;
            }
        }
        // Cursor on first non-blank of the first shifted line.
        let line = selection::line_range_at(buffer.rope(), buffer.rope().line_to_char(first));
        buffer.set_cursor_raw(first_non_blank(buffer.rope(), line.start, line.end));
        buffer.clear_selection();
        self.clear_pending();
        self.mode = Mode::Normal;
        if any { edited() } else { handled(false) }
    }
}

pub(in crate::vim) fn first_non_blank(rope: &ropey::Rope, start: usize, end: usize) -> usize {
    let end = end.min(rope.len_chars());
    let mut pos = start.min(end);
    while pos < end {
        let c = rope.char(pos);
        if c == '\n' {
            break;
        }
        if c != ' ' && c != '\t' {
            return pos;
        }
        pos += 1;
    }
    start.min(end)
}

/// Spaces inserted by `>>` (no `shiftwidth` setting yet).
const SHIFTWIDTH: usize = 4;

fn join_once(buffer: &mut Buffer, space: bool) -> Option<usize> {
    let rope = buffer.rope();
    let len = rope.len_chars();
    if len == 0 {
        return None;
    }
    let cursor = buffer.cursor().min(len.saturating_sub(1));
    let row = rope.char_to_line(cursor);
    if row + 1 >= rope.len_lines() {
        return None;
    }
    let cur = selection::line_range_at(rope, cursor);
    // Need a newline between lines.
    if cur.end >= len || rope.char(cur.end) != '\n' {
        // Last physical line with no trailing newline, or empty.
        return None;
    }
    let next_start = cur.end + 1;
    if next_start > len {
        return None;
    }
    let next = selection::line_range_at(rope, next_start.min(len.saturating_sub(1)));
    // Leading whitespace on the next line (to strip).
    let mut body = next.start;
    while body < next.end {
        let c = rope.char(body);
        if c != ' ' && c != '\t' {
            break;
        }
        body += 1;
    }
    let left_empty = cur.start == cur.end
        || (cur.end > cur.start && {
            let last = rope.char(cur.end - 1);
            last == ' ' || last == '\t'
        });
    let right_empty = body >= next.end;
    let insert_space = space && !left_empty && !right_empty;
    // Delete from newline through leading ws of next; maybe insert a space.
    let del_start = cur.end;
    let del_end = body;
    let old = rope.slice(del_start..del_end).to_string();
    let new = if insert_space {
        " ".to_string()
    } else {
        String::new()
    };
    buffer.apply_edit(Edit {
        start: del_start,
        old,
        new,
    });
    // Cursor on the join point (space, or first char of the former next line).
    Some(del_start)
}

fn line_indent(rope: &ropey::Rope, row: usize) -> String {
    if row >= rope.len_lines() {
        return String::new();
    }
    let line = selection::line_range_at(rope, rope.line_to_char(row));
    let mut indent = String::new();
    for i in line.start..line.end {
        let c = rope.char(i);
        if c == ' ' || c == '\t' {
            indent.push(c);
        } else {
            break;
        }
    }
    indent
}

/// Shift one line. Returns whether the buffer changed.
fn shift_line(buffer: &mut Buffer, row: usize, op: Operator, prev_indent: &str) -> bool {
    let rope = buffer.rope();
    if row >= rope.len_lines() {
        return false;
    }
    let line = selection::line_range_at(rope, rope.line_to_char(row));
    // Skip blank lines for indent/outdent (vim leaves them alone).
    let is_blank = (line.start..line.end).all(|i| {
        let c = rope.char(i);
        c == ' ' || c == '\t'
    });
    match op {
        Operator::Indent if is_blank => false,
        Operator::Outdent if is_blank => false,
        Operator::Indent => {
            let pad = " ".repeat(SHIFTWIDTH);
            buffer.apply_edit(Edit {
                start: line.start,
                old: String::new(),
                new: pad,
            });
            true
        }
        Operator::Outdent => {
            let mut remove = 0usize;
            let i = line.start;
            // Prefer removing a leading tab, else up to SHIFTWIDTH spaces.
            if i < line.end && rope.char(i) == '\t' {
                remove = 1;
            } else {
                while i + remove < line.end && remove < SHIFTWIDTH && rope.char(i + remove) == ' ' {
                    remove += 1;
                }
            }
            if remove == 0 {
                return false;
            }
            let old = rope.slice(line.start..line.start + remove).to_string();
            buffer.apply_edit(Edit {
                start: line.start,
                old,
                new: String::new(),
            });
            true
        }
        Operator::Reindent => {
            // Match previous line indent; blank lines stay blank.
            if is_blank {
                return false;
            }
            let mut indent_end = line.start;
            while indent_end < line.end {
                let c = rope.char(indent_end);
                if c != ' ' && c != '\t' {
                    break;
                }
                indent_end += 1;
            }
            let old = rope.slice(line.start..indent_end).to_string();
            if old == prev_indent {
                return false;
            }
            buffer.apply_edit(Edit {
                start: line.start,
                old,
                new: prev_indent.to_string(),
            });
            true
        }
        _ => false,
    }
}

/// Number at or after `cursor` on the same line. Supports optional leading `-`.
fn find_number(rope: &ropey::Rope, cursor: usize) -> Option<(std::ops::Range<usize>, i64)> {
    let len = rope.len_chars();
    if len == 0 {
        return None;
    }
    let cursor = cursor.min(len.saturating_sub(1));
    let line = selection::line_range_at(rope, cursor);
    let digit_at = |i: usize| i < line.end && rope.char(i).is_ascii_digit();

    // Start of the digit run: under cursor, or first digit to the right on this line.
    let mut digits = if digit_at(cursor) {
        let mut s = cursor;
        while s > line.start && rope.char(s - 1).is_ascii_digit() {
            s -= 1;
        }
        s
    } else {
        let mut i = cursor;
        while i < line.end && !rope.char(i).is_ascii_digit() {
            i += 1;
        }
        if i >= line.end {
            return None;
        }
        i
    };

    // Optional leading `-` immediately before the digits.
    if digits > line.start && rope.char(digits - 1) == '-' {
        digits -= 1;
    }

    let mut end = digits;
    if end < line.end && rope.char(end) == '-' {
        end += 1;
    }
    let digit_start = end;
    while end < line.end && rope.char(end).is_ascii_digit() {
        end += 1;
    }
    if end == digit_start {
        return None;
    }
    let text = rope.slice(digits..end).to_string();
    let value: i64 = text.parse().ok()?;
    Some((digits..end, value))
}

/// Keep similar width when the old number had leading zeros (e.g. `007` → `008`).
fn preserve_width(old: &str, new: &str) -> String {
    let old_digits = old.trim_start_matches('-');
    let new_digits = new.trim_start_matches('-');
    if old_digits.len() > new_digits.len() && old_digits.starts_with('0') {
        let body = format!("{:0>width$}", new_digits, width = old_digits.len());
        if new.starts_with('-') {
            format!("-{body}")
        } else {
            body
        }
    } else {
        new.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Mode, VimState};
    use crate::buffer::Buffer;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn buffer_with(text: &str) -> Buffer {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        Buffer::open(file.path()).unwrap()
    }

    #[test]
    fn join_lines_inserts_space() {
        let mut buffer = buffer_with("hello\nworld\n");
        let mut vim = VimState::default();
        assert!(vim.handle_char(&mut buffer, "J").edited);
        assert_eq!(buffer.text(), "hello world\n");
    }

    #[test]
    fn gj_joins_without_space() {
        let mut buffer = buffer_with("hello\nworld\n");
        let mut vim = VimState::default();
        vim.handle_char(&mut buffer, "g");
        assert!(vim.handle_char(&mut buffer, "J").edited);
        assert_eq!(buffer.text(), "helloworld\n");
    }

    #[test]
    fn d_c_y_s_shortcuts() {
        let mut buffer = buffer_with("hello\nworld\n");
        let mut vim = VimState::default();
        // D = d$ from start → empty first line
        assert!(vim.handle_char(&mut buffer, "D").edited);
        assert_eq!(buffer.text(), "\nworld\n");

        let mut buffer = buffer_with("hello\nworld\n");
        let mut vim = VimState::default();
        // Y = yy, then j, p pastes below
        vim.handle_char(&mut buffer, "Y");
        vim.handle_char(&mut buffer, "j");
        assert!(vim.handle_char(&mut buffer, "p").edited);
        assert_eq!(buffer.text(), "hello\nworld\nhello\n");

        let mut buffer = buffer_with("hello\nworld\n");
        let mut vim = VimState::default();
        // S = cc → delete line, insert at start of what follows.
        assert!(vim.handle_char(&mut buffer, "S").edited);
        assert_eq!(vim.mode, Mode::Insert);
        assert_eq!(buffer.text(), "world\n");
    }

    #[test]
    fn indent_outdent_shiftwidth_four() {
        let mut buffer = buffer_with("foo\n");
        let mut vim = VimState::default();
        vim.handle_char(&mut buffer, ">");
        assert!(vim.handle_char(&mut buffer, ">").edited);
        assert_eq!(buffer.text(), "    foo\n");
        vim.handle_char(&mut buffer, "<");
        assert!(vim.handle_char(&mut buffer, "<").edited);
        assert_eq!(buffer.text(), "foo\n");
    }

    #[test]
    fn ctrl_a_increments_number() {
        let mut buffer = buffer_with("val 41 end\n");
        let mut vim = VimState::default();
        // Cursor on 'v'; Ctrl-a finds 41 after.
        assert!(vim.change_number(&mut buffer, 1, 1).edited);
        assert_eq!(buffer.text(), "val 42 end\n");
    }
}
