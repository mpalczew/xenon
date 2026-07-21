//! Operators, paste, and motion application for vim mode.

use super::motion;
use super::object::Object;
use super::repeat::LastChange;
use super::{HandleResult, Mode, Motion, Operator, VimState, edited, handled};
use crate::buffer::Buffer;
use crate::selection;
use crate::undo::Edit;

impl VimState {
    pub(in crate::vim) fn op_or_line(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        count: usize,
        same: char,
    ) -> HandleResult {
        if self.mode.is_visual() {
            return self.visual_operator(buffer, op);
        }
        if self.operator == Some(op) {
            // dd / cc / yy
            self.operator = None;
            let lines = count.max(1);
            let reg = self.registers.pending();
            // Yank is not a "change" for `.`; delete/change are.
            if op != Operator::Yank {
                self.last_change = Some(LastChange::Lines {
                    op,
                    count: lines,
                    register: reg,
                });
            }
            self.count = 0;
            let range = linewise_range(buffer, lines);
            return self.apply_range(buffer, op, range, true);
        }
        let _ = same;
        self.operator = Some(op);
        // Keep self.count so `3dw` works (count was set by digits before the op).
        if self.count == 0 {
            // no-op: operator pending with implicit count 1
        }
        handled(false)
    }

    pub(in crate::vim) fn do_motion(
        &mut self,
        buffer: &mut Buffer,
        motion: Motion,
        count: usize,
    ) -> HandleResult {
        self.finish_motion(buffer, motion, count)
    }

    pub(in crate::vim) fn finish_motion(
        &mut self,
        buffer: &mut Buffer,
        motion: Motion,
        count: usize,
    ) -> HandleResult {
        let count = count.max(1);
        if let Some(op) = self.operator.take() {
            let op_count = if self.count > 0 && self.count != usize::MAX {
                self.count
            } else {
                count
            };
            self.count = 0;
            let range = motion::operator_range(buffer.rope(), buffer.cursor(), &motion, op_count);
            let reg = self.registers.pending();
            self.last_change = Some(LastChange::Operator {
                op,
                motion: motion.clone(),
                count: op_count,
                register: reg,
            });
            return self.apply_range(buffer, op, range, motion.is_linewise());
        }
        if self.mode.is_visual() {
            // Motions run from the inclusive head (cursor is exclusive end in char visual).
            let from = if self.mode == Mode::Visual {
                visual_head_pos(buffer)
            } else {
                buffer.cursor()
            };
            let target = motion::apply(buffer.rope(), from, &motion, count);
            let anchor = buffer.selection_anchor().unwrap_or(from);
            if self.mode == Mode::VisualLine {
                let a = selection::line_range_at(buffer.rope(), anchor.min(from));
                let b = selection::line_range_at(buffer.rope(), target);
                let start = a.start.min(b.start);
                let mut end = a.end.max(b.end);
                let len = buffer.rope().len_chars();
                if end < len && buffer.rope().char(end) == '\n' {
                    end += 1;
                }
                buffer.set_selection(start, end);
            } else {
                // Half-open range; fixed inclusive start is prior.start (or current head).
                let prior = buffer.selection_range();
                let fixed = prior.as_ref().map(|r| r.start).unwrap_or(from);
                let excl = visual_excl_end(buffer, target);
                if excl <= fixed {
                    // Moving left of fixed: select target..=fixed.
                    let fixed_excl = visual_excl_end(buffer, fixed);
                    buffer.set_selection(target, fixed_excl);
                } else {
                    buffer.set_selection(fixed, excl);
                }
                let _ = anchor;
            }
        } else {
            let target = motion::apply(buffer.rope(), buffer.cursor(), &motion, count);
            buffer.clear_selection();
            buffer.set_cursor_raw(target);
        }
        self.count = 0;
        handled(false)
    }

    pub(in crate::vim) fn apply_object(
        &mut self,
        buffer: &mut Buffer,
        object: Object,
    ) -> HandleResult {
        let Some(op) = self.operator.take() else {
            // Visual object expand
            if let Some(range) = object.range(buffer.rope(), buffer.cursor()) {
                buffer.set_selection(range.start, range.end);
                self.mode = Mode::Visual;
            }
            return handled(false);
        };
        let reg = self.registers.pending();
        let Some(range) = object.range(buffer.rope(), buffer.cursor()) else {
            self.clear_pending();
            return handled(false);
        };
        self.last_change = Some(LastChange::Object {
            op,
            object,
            register: reg,
        });
        self.apply_range(buffer, op, range, false)
    }

    pub(in crate::vim) fn visual_operator(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
    ) -> HandleResult {
        let Some(range) = buffer.selection_range() else {
            self.mode = Mode::Normal;
            return handled(false);
        };
        let result = self.apply_range(buffer, op, range, self.mode == Mode::VisualLine);
        self.mode = Mode::Normal;
        result
    }

    pub(in crate::vim) fn apply_range(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        range: std::ops::Range<usize>,
        linewise: bool,
    ) -> HandleResult {
        if range.start >= range.end {
            self.clear_pending();
            return handled(false);
        }
        let text = buffer.rope().slice(range.clone()).to_string();
        let mut system_clipboard = None;
        let edited = match op {
            Operator::Yank => {
                if matches!(self.registers.pending(), Some('+' | '*')) {
                    system_clipboard = Some(text.clone());
                }
                self.registers.yank(&text, linewise);
                buffer.set_cursor_raw(range.start);
                buffer.clear_selection();
                self.clear_pending();
                false
            }
            Operator::Delete | Operator::Change => {
                if matches!(self.registers.pending(), Some('+' | '*')) {
                    system_clipboard = Some(text.clone());
                }
                self.registers.delete(&text, linewise);
                // `c` delete + insert until Esc is one undo step (vim).
                if op == Operator::Change {
                    buffer.set_undo_group(true);
                }
                buffer.set_selection(range.start, range.end);
                buffer.delete_selection();
                self.clear_pending();
                if op == Operator::Change {
                    self.enter_insert(buffer);
                } else {
                    self.mode = Mode::Normal;
                }
                true
            }
        };
        HandleResult {
            handled: true,
            edited,
            system_clipboard,
            ..Default::default()
        }
    }

    pub(in crate::vim) fn delete_chars(
        &mut self,
        buffer: &mut Buffer,
        count: usize,
        backward: bool,
    ) -> HandleResult {
        self.last_change = Some(LastChange::DeleteChar { count });
        let cursor = buffer.cursor();
        let (start, end) = if backward {
            (cursor.saturating_sub(count), cursor)
        } else {
            let end = (cursor + count).min(buffer.rope().len_chars());
            (cursor, end)
        };
        if start == end {
            return handled(false);
        }
        let text = buffer.rope().slice(start..end).to_string();
        self.registers.delete(&text, false);
        buffer.set_selection(start, end);
        buffer.delete_selection();
        self.count = 0;
        edited()
    }

    pub(in crate::vim) fn replace_char(&mut self, buffer: &mut Buffer, ch: char) -> HandleResult {
        if self.mode.is_visual() {
            let Some(range) = buffer.selection_range() else {
                self.mode = Mode::Normal;
                return handled(false);
            };
            let len = range.end.saturating_sub(range.start);
            if len == 0 {
                return handled(false);
            }
            let new: String = std::iter::repeat_n(ch, len).collect();
            let old = buffer.rope().slice(range.clone()).to_string();
            buffer.apply_edit(Edit {
                start: range.start,
                old,
                new,
            });
            buffer.set_cursor_raw(range.start);
            buffer.clear_selection();
            self.mode = Mode::Normal;
            self.last_change = Some(LastChange::Replace { ch });
            return edited();
        }
        let cursor = buffer.cursor();
        if cursor >= buffer.rope().len_chars() {
            return handled(false);
        }
        let old = buffer.rope().slice(cursor..cursor + 1).to_string();
        buffer.apply_edit(Edit {
            start: cursor,
            old,
            new: ch.to_string(),
        });
        buffer.set_cursor_raw(cursor);
        self.last_change = Some(LastChange::Replace { ch });
        edited()
    }

    /// Swap visual anchor and head (`o` in visual).
    pub(in crate::vim) fn visual_swap_ends(&mut self, buffer: &mut Buffer) -> HandleResult {
        if let (Some(anchor), cursor) = (buffer.selection_anchor(), buffer.cursor()) {
            buffer.set_selection(cursor, anchor);
        }
        handled(false)
    }

    /// Visual `p`/`P`: replace selection with register contents.
    pub(in crate::vim) fn visual_paste(
        &mut self,
        buffer: &mut Buffer,
        before: bool,
    ) -> HandleResult {
        let Some(range) = buffer.selection_range() else {
            self.mode = Mode::Normal;
            return handled(false);
        };
        let deleted = buffer.rope().slice(range.clone()).to_string();
        self.registers
            .delete(&deleted, self.mode == Mode::VisualLine);
        buffer.set_selection(range.start, range.end);
        buffer.delete_selection();
        self.mode = Mode::Normal;
        self.paste(buffer, before)
    }

    pub(in crate::vim) fn paste(&mut self, buffer: &mut Buffer, before: bool) -> HandleResult {
        if matches!(self.registers.pending(), Some('+' | '*')) {
            return HandleResult {
                handled: true,
                request_system_paste: true,
                ..Default::default()
            };
        }
        let content = self.registers.paste();
        if content.text.is_empty() {
            return handled(false);
        }
        self.last_change = Some(LastChange::Paste { before });
        if content.linewise {
            return paste_linewise(buffer, &content.text, before);
        }
        paste_charwise(buffer, &content.text, before)
    }

    /// Paste system clipboard text (view resolved `"+`). Characterwise.
    pub fn paste_system(&mut self, buffer: &mut Buffer, text: &str, before: bool) -> HandleResult {
        self.registers.set_unnamed(text);
        self.last_change = Some(LastChange::Paste { before });
        paste_charwise(buffer, text, before)
    }

    /// Re-apply a linewise `dd` / `cc` (used by `.`).
    pub(in crate::vim) fn repeat_lines(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        count: usize,
        register: Option<char>,
    ) -> HandleResult {
        if let Some(r) = register {
            self.registers.set_pending(r);
        }
        let range = linewise_range(buffer, count.max(1));
        self.apply_range(buffer, op, range, true)
    }
}

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
fn visual_head_pos(buffer: &Buffer) -> usize {
    let c = buffer.cursor();
    if c > 0 { c - 1 } else { c }
}

/// Char range for `count` lines starting at the cursor (includes trailing newlines).
fn linewise_range(buffer: &Buffer, count: usize) -> std::ops::Range<usize> {
    let mut range = selection::line_range_at(buffer.rope(), buffer.cursor());
    for _ in 1..count {
        if range.end >= buffer.rope().len_chars() {
            break;
        }
        let next = selection::line_range_at(buffer.rope(), range.end);
        range.end = next.end;
    }
    if range.end < buffer.rope().len_chars() && buffer.rope().char(range.end) == '\n' {
        range.end += 1;
    }
    range
}

fn paste_charwise(buffer: &mut Buffer, text: &str, before: bool) -> HandleResult {
    if !before && buffer.cursor() < buffer.rope().len_chars() {
        buffer.set_cursor_raw(buffer.cursor() + 1);
    }
    buffer.replace_selection(text);
    edited()
}

/// Linewise `p` / `P`: whole lines below / above the current line.
fn paste_linewise(buffer: &mut Buffer, text: &str, before: bool) -> HandleResult {
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

/// First non-blank char in `[start, end)`, or `start` if the span is blank.
fn first_non_blank(rope: &ropey::Rope, start: usize, end: usize) -> usize {
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
