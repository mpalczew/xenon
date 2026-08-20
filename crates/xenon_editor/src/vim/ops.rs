//! Operators, paste, and motion application for vim mode.

use super::block;
use super::block_ops::paste_blockwise;
use super::motion;
use super::object::Object;
use super::paste::{
    lines_covering, paste_charwise, paste_linewise, visual_excl_end, visual_head_pos,
};
use super::repeat::LastChange;
use super::{HandleResult, Mode, Motion, Operator, VimState, edited, handled};
use crate::buffer::Buffer;
use crate::selection;
use crate::undo::Edit;

impl VimState {
    /// Start an operator; stash the prefix count so `2dw` / `2d3w` can multiply.
    pub(in crate::vim) fn begin_operator(&mut self, op: Operator) {
        self.operator_count = self.count.max(1);
        self.count = 0;
        self.operator = Some(op);
    }

    pub(in crate::vim) fn op_or_line(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        count: usize,
    ) -> HandleResult {
        if self.mode.is_visual() {
            return self.visual_operator(buffer, op);
        }
        if self.operator == Some(op) {
            // dd / cc / yy / >> / << / == — same as operator + line motion `_`.
            return self.finish_motion(buffer, Motion::Line, count);
        }
        self.begin_operator(op);
        handled(false)
    }

    /// `Y` / `S`: operator + line motion with the prefix count (`2Y` = two lines).
    pub(in crate::vim) fn apply_linewise_now(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
    ) -> HandleResult {
        if self.mode.is_visual() {
            return self.visual_operator(buffer, op);
        }
        if self.operator != Some(op) {
            self.begin_operator(op);
        }
        self.finish_motion(buffer, Motion::Line, 1)
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
            let op_count = self.operator_count.max(1);
            self.operator_count = 0;
            self.count = 0;
            let effective = op_count.saturating_mul(count);
            // Indent family is always linewise in classic vim (`>w` still whole lines).
            let force_line = matches!(
                op,
                Operator::Indent | Operator::Outdent | Operator::Reindent
            );
            let range = if force_line {
                let target = motion::apply(buffer.rope(), buffer.cursor(), &motion, effective);
                lines_covering(buffer, buffer.cursor(), target)
            } else {
                motion::operator_range(buffer.rope(), buffer.cursor(), &motion, effective)
            };
            let reg = self.registers.pending();
            if op != Operator::Yank {
                self.last_change = Some(LastChange::Operator {
                    op,
                    motion: motion.clone(),
                    count: effective,
                    register: reg,
                });
            }
            return self.apply_range(buffer, op, range, force_line || motion.is_linewise());
        }
        if self.mode == Mode::VisualBlock {
            // Head moves; anchor corner stays put. Desired column is free to go past EOL.
            let target = motion::apply(buffer.rope(), buffer.cursor(), &motion, count);
            buffer.set_cursor_raw(target);
            buffer.clear_selection();
        } else if self.mode.is_visual() {
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
        if self.mode == Mode::VisualBlock {
            return self.block_operator(buffer, op);
        }
        let Some(range) = buffer.selection_range() else {
            self.mode = Mode::Normal;
            self.block_anchor = None;
            return handled(false);
        };
        let result = self.apply_range(buffer, op, range, self.mode == Mode::VisualLine);
        self.mode = Mode::Normal;
        self.block_anchor = None;
        result
    }

    /// Visual-block `d`/`c`/`y`/`>`/`<`/`=`.
    pub(in crate::vim) fn apply_range(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
        range: std::ops::Range<usize>,
        linewise: bool,
    ) -> HandleResult {
        if matches!(
            op,
            Operator::Indent | Operator::Outdent | Operator::Reindent
        ) {
            return self.shift_range(buffer, op, range);
        }
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
            Operator::Indent | Operator::Outdent | Operator::Reindent => unreachable!(),
        };
        HandleResult {
            handled: true,
            edited,
            system_clipboard,
            ..Default::default()
        }
    }

    /// `J` / `gJ`: fuse `lines` lines starting at the cursor (≥2).
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
        if self.mode == Mode::VisualBlock {
            let Some(corners) = self.block_corners(buffer) else {
                self.mode = Mode::Normal;
                return handled(false);
            };
            let ranges = block::char_ranges(buffer.rope(), corners);
            if ranges.is_empty() {
                self.mode = Mode::Normal;
                self.block_anchor = None;
                return handled(false);
            }
            for range in ranges.into_iter().rev() {
                let len = range.end.saturating_sub(range.start);
                if len == 0 {
                    continue;
                }
                let new: String = std::iter::repeat_n(ch, len).collect();
                let old = buffer.rope().slice(range.clone()).to_string();
                buffer.apply_edit(Edit {
                    start: range.start,
                    old,
                    new,
                });
            }
            let pos = block::offset_at(buffer.rope(), corners.min_row, corners.min_col);
            buffer.set_cursor_raw(pos);
            buffer.clear_selection();
            self.block_anchor = None;
            self.mode = Mode::Normal;
            self.last_change = Some(LastChange::Replace { ch });
            return edited();
        }
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
        if self.mode == Mode::VisualBlock {
            if let Some(anchor) = self.block_anchor {
                let head = buffer.cursor_position();
                self.block_anchor = Some(head);
                buffer.set_cursor_position(anchor.0, anchor.1);
            }
            return handled(false);
        }
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
        if self.mode == Mode::VisualBlock {
            // Paste register into the block: keep register, delete to black hole.
            let content = self.registers.paste();
            self.registers.set_pending('_');
            let _ = self.block_operator(buffer, Operator::Delete);
            if content.text.is_empty() {
                return handled(false);
            }
            self.last_change = Some(LastChange::Paste { before: true });
            if content.blockwise {
                return paste_blockwise(buffer, &content.text, true);
            }
            if content.linewise {
                return paste_linewise(buffer, &content.text, true);
            }
            return paste_charwise(buffer, &content.text, true);
        }
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
        self.block_anchor = None;
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
        if content.blockwise {
            return paste_blockwise(buffer, &content.text, before);
        }
        if content.linewise {
            return paste_linewise(buffer, &content.text, before);
        }
        paste_charwise(buffer, &content.text, before)
    }

    /// Paste system clipboard text (view resolved `"+`). Characterwise.
    pub(crate) fn paste_system(
        &mut self,
        buffer: &mut Buffer,
        text: &str,
        before: bool,
    ) -> HandleResult {
        self.registers.set_unnamed(text);
        self.last_change = Some(LastChange::Paste { before });
        paste_charwise(buffer, text, before)
    }
}
