//! Paste and visual selection helpers for vim operators.

use super::edits::first_non_blank;
use super::{HandleResult, edited};
use crate::buffer::Buffer;
use crate::selection;

/// Exclusive end past char at `pos` (vim visual includes that char).
pub(in crate::vim) fn visual_excl_end(buffer: &Buffer, pos: usize) -> usize {
    let len = buffer.rope().len_chars();
    if pos < len && buffer.rope().char(pos) != '\n' {
        pos + 1
    } else {
        pos
    }
}

/// Inclusive head character for a half-open visual selection.
pub(in crate::vim) fn visual_head_pos(buffer: &Buffer) -> usize {
    let c = buffer.cursor();
    if c > 0 { c - 1 } else { c }
}

pub(in crate::vim) fn paste_charwise(
    buffer: &mut Buffer,
    text: &str,
    before: bool,
) -> HandleResult {
    if !before && buffer.cursor() < buffer.rope().len_chars() {
        buffer.set_cursor_raw(buffer.cursor() + 1);
    }
    buffer.replace_selection(text);
    edited()
}

pub(in crate::vim) fn paste_linewise(
    buffer: &mut Buffer,
    text: &str,
    before: bool,
) -> HandleResult {
    let mut text = text.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let cursor = buffer.cursor();
    let line = selection::line_range_at(buffer.rope(), cursor);
    let len = buffer.rope().len_chars();
    let (insert_at, body_start) = if before {
        (line.start, line.start)
    } else if line.end < len && buffer.rope().char(line.end) == '\n' {
        let at = line.end + 1;
        (at, at)
    } else {
        // Last line has no trailing newline: open a new line below first.
        text.insert(0, '\n');
        (line.end, line.end + 1)
    };
    buffer.clear_selection();
    buffer.set_cursor_raw(insert_at);
    buffer.replace_selection(&text);
    let body_end = insert_at + text.chars().count();
    buffer.set_cursor_raw(first_non_blank(buffer.rope(), body_start, body_end));
    edited()
}

/// Full lines covering two char positions (for indent operators).
pub(in crate::vim) fn lines_covering(
    buffer: &Buffer,
    a: usize,
    b: usize,
) -> std::ops::Range<usize> {
    let lo = a.min(b);
    let hi = a.max(b);
    let start = selection::line_range_at(buffer.rope(), lo).start;
    let mut end = selection::line_range_at(buffer.rope(), hi).end;
    let len = buffer.rope().len_chars();
    if end < len && buffer.rope().char(end) == '\n' {
        end += 1;
    }
    start..end
}

/// Highlight `[pos, pos+len)` and leave the cursor on the first match char.
pub(in crate::vim) fn highlight_match(buffer: &mut Buffer, pos: usize, len: usize) {
    let end = (pos + len).min(buffer.rope().len_chars());
    buffer.set_selection(end, pos);
}
