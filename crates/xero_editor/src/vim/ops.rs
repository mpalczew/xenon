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
            let mut range = selection::line_range_at(buffer.rope(), buffer.cursor());
            for _ in 1..lines {
                if range.end >= buffer.rope().len_chars() {
                    break;
                }
                let next = selection::line_range_at(buffer.rope(), range.end);
                range.end = next.end;
            }
            if range.end < buffer.rope().len_chars() && buffer.rope().char(range.end) == '\n' {
                range.end += 1;
            }
            self.count = 0;
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
        let target = motion::apply(buffer.rope(), buffer.cursor(), &motion, count);
        if self.mode.is_visual() {
            let anchor = buffer.selection_anchor().unwrap_or(buffer.cursor());
            if self.mode == Mode::VisualLine {
                let a = selection::line_range_at(buffer.rope(), anchor);
                let b = selection::line_range_at(buffer.rope(), target);
                buffer.set_selection(a.start.min(b.start), a.end.max(b.end));
            } else {
                buffer.set_selection(anchor, target);
            }
        } else {
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
        _linewise: bool,
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
                self.registers.yank(&text);
                buffer.set_cursor_raw(range.start);
                buffer.clear_selection();
                self.clear_pending();
                false
            }
            Operator::Delete | Operator::Change => {
                if matches!(self.registers.pending(), Some('+' | '*')) {
                    system_clipboard = Some(text.clone());
                }
                self.registers.delete(&text);
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
        self.registers.delete(&text);
        buffer.set_selection(start, end);
        buffer.delete_selection();
        self.count = 0;
        edited()
    }

    pub(in crate::vim) fn replace_char(&mut self, buffer: &mut Buffer, ch: char) -> HandleResult {
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

    pub(in crate::vim) fn paste(&mut self, buffer: &mut Buffer, before: bool) -> HandleResult {
        if matches!(self.registers.pending(), Some('+' | '*')) {
            return HandleResult {
                handled: true,
                request_system_paste: true,
                ..Default::default()
            };
        }
        let text = self.registers.paste_text();
        if text.is_empty() {
            return handled(false);
        }
        if !before && buffer.cursor() < buffer.rope().len_chars() {
            buffer.set_cursor_raw(buffer.cursor() + 1);
        }
        buffer.replace_selection(&text);
        edited()
    }

    /// Paste system clipboard text (view resolved `"+`).
    pub fn paste_system(&mut self, buffer: &mut Buffer, text: &str, before: bool) -> HandleResult {
        self.registers.set_unnamed(text);
        if !before && buffer.cursor() < buffer.rope().len_chars() {
            buffer.set_cursor_raw(buffer.cursor() + 1);
        }
        buffer.replace_selection(text);
        edited()
    }
}
