//! Visual-block operators, insert, and paste.

use super::block;
use super::{BlockInsert, HandleResult, Mode, Operator, VimState, edited, handled};
use crate::buffer::Buffer;
use crate::selection;

impl VimState {
    pub(in crate::vim) fn block_operator(
        &mut self,
        buffer: &mut Buffer,
        op: Operator,
    ) -> HandleResult {
        let Some(corners) = self.block_corners(buffer) else {
            self.mode = Mode::Normal;
            self.block_anchor = None;
            return handled(false);
        };
        // Indent family acts on whole lines covered by the block.
        if matches!(
            op,
            Operator::Indent | Operator::Outdent | Operator::Reindent
        ) {
            let range = block::line_span(buffer.rope(), corners);
            self.block_anchor = None;
            let result = self.shift_range(buffer, op, range);
            self.mode = Mode::Normal;
            return result;
        }
        let text = block::block_text(buffer.rope(), corners);
        let ranges = block::char_ranges(buffer.rope(), corners);
        let mut system_clipboard = None;
        match op {
            Operator::Yank => {
                if matches!(self.registers.pending(), Some('+' | '*')) {
                    system_clipboard = Some(text.clone());
                }
                self.registers.yank_block(&text);
                let pos = block::offset_at(buffer.rope(), corners.min_row, corners.min_col);
                buffer.set_cursor_raw(pos);
                buffer.clear_selection();
                self.block_anchor = None;
                self.mode = Mode::Normal;
                self.clear_pending();
                HandleResult {
                    handled: true,
                    system_clipboard,
                    ..Default::default()
                }
            }
            Operator::Delete | Operator::Change => {
                if matches!(self.registers.pending(), Some('+' | '*')) {
                    system_clipboard = Some(text.clone());
                }
                self.registers.delete_block(&text);
                if op == Operator::Change {
                    buffer.set_undo_group(true);
                }
                // Delete bottom-up so earlier offsets stay valid.
                for range in ranges.into_iter().rev() {
                    if range.start < range.end {
                        buffer.replace_range(range, "");
                    }
                }
                let pos = block::offset_at(buffer.rope(), corners.min_row, corners.min_col);
                buffer.set_cursor_raw(pos);
                buffer.clear_selection();
                self.block_anchor = None;
                self.clear_pending();
                if op == Operator::Change {
                    self.enter_insert(buffer);
                } else {
                    self.mode = Mode::Normal;
                }
                HandleResult {
                    handled: true,
                    edited: true,
                    system_clipboard,
                    ..Default::default()
                }
            }
            Operator::Indent | Operator::Outdent | Operator::Reindent => unreachable!(),
        }
    }

    /// Visual-block `I` (before) / `A` (after): insert, then replicate on Esc.
    pub(in crate::vim) fn block_insert(
        &mut self,
        buffer: &mut Buffer,
        before: bool,
    ) -> HandleResult {
        let Some(corners) = self.block_corners(buffer) else {
            return handled(false);
        };
        let col = if before {
            corners.min_col
        } else {
            corners.max_col + 1
        };
        let typed_row = corners.min_row;
        let other_rows: Vec<usize> = (corners.min_row..=corners.max_row)
            .filter(|&r| r != typed_row)
            .collect();
        // Place cursor on top line at insert column (pad if past EOL).
        let rope = buffer.rope();
        let line_start = rope.line_to_char(typed_row.min(rope.len_lines().saturating_sub(1)));
        let len = block::line_content_len(rope, typed_row);
        if col > len {
            // Pad top line so insert sits at the block column.
            let pad: String = std::iter::repeat_n(' ', col - len).collect();
            buffer.set_cursor_raw(line_start + len);
            buffer.clear_selection();
            buffer.replace_selection(&pad);
        }
        let at = block::offset_at(buffer.rope(), typed_row, col);
        buffer.set_cursor_raw(at);
        buffer.clear_selection();
        self.block_anchor = None;
        self.block_insert = Some(BlockInsert { other_rows, col });
        self.enter_insert(buffer);
        handled(false)
    }

    /// Half-open char ranges to paint for the current selection (block = multi).
    pub(crate) fn paint_selection_ranges(&self, buffer: &Buffer) -> Vec<std::ops::Range<usize>> {
        if self.mode == Mode::VisualBlock
            && let Some(anchor) = self.block_anchor
        {
            let head = buffer.cursor_position();
            let corners = block::BlockCorners::from_corners(anchor, head);
            return block::char_ranges(buffer.rope(), corners);
        }
        buffer.selection_range().into_iter().collect()
    }

    /// Current block corners when in visual-block mode.
    pub(in crate::vim) fn block_corners(&self, buffer: &Buffer) -> Option<block::BlockCorners> {
        let anchor = self.block_anchor?;
        if self.mode != Mode::VisualBlock {
            return None;
        }
        Some(block::BlockCorners::from_corners(
            anchor,
            buffer.cursor_position(),
        ))
    }

    /// Enter or leave visual-block (`Ctrl-v`).
    pub(crate) fn toggle_visual_block(&mut self, buffer: &mut Buffer) -> HandleResult {
        if self.mode == Mode::VisualBlock {
            buffer.clear_selection();
            self.block_anchor = None;
            self.mode = Mode::Normal;
            self.clear_pending();
            return handled(false);
        }
        if self.mode.is_visual()
            && let Some(range) = buffer.selection_range()
        {
            let rope = buffer.rope();
            let start = range.start.min(rope.len_chars().saturating_sub(1));
            let end = range
                .end
                .saturating_sub(1)
                .min(rope.len_chars().saturating_sub(1));
            let a = selection::offset_line_col(rope, start);
            let b = selection::offset_line_col(rope, end);
            self.block_anchor = Some((a.0 as usize, a.1 as usize));
            buffer.set_cursor_position(b.0 as usize, b.1 as usize);
        } else {
            self.block_anchor = Some(buffer.cursor_position());
        }
        buffer.clear_selection();
        self.mode = Mode::VisualBlock;
        self.clear_pending();
        handled(false)
    }
}

/// Blockwise paste: put each line of `text` at the same column on successive rows.
pub(in crate::vim) fn paste_blockwise(
    buffer: &mut Buffer,
    text: &str,
    before: bool,
) -> HandleResult {
    let parts: Vec<&str> = text.split('\n').collect();
    if parts.is_empty() {
        return handled(false);
    }
    let (start_row, mut col) = buffer.cursor_position();
    if !before {
        col = col.saturating_add(1);
    }
    let need_rows = start_row + parts.len();
    // Ensure enough lines exist (rope always has ≥1 line).
    while buffer.rope().len_lines() < need_rows {
        let len = buffer.rope().len_chars();
        buffer.set_cursor_raw(len);
        buffer.clear_selection();
        buffer.replace_selection("\n");
    }
    // Bottom-up so earlier row inserts do not shift later targets.
    for (i, part) in parts.iter().enumerate().rev() {
        let row = start_row + i;
        let line_start = buffer.rope().line_to_char(row);
        let len = block::line_content_len(buffer.rope(), row);
        let mut at = line_start + col.min(len);
        let mut chunk = String::new();
        if col > len {
            chunk.extend(std::iter::repeat_n(' ', col - len));
            at = line_start + len;
        }
        chunk.push_str(part);
        buffer.set_cursor_raw(at);
        buffer.clear_selection();
        buffer.replace_selection(&chunk);
    }
    buffer.set_cursor_position(start_row, col);
    edited()
}

/// Insert `text` at `col` on each row in `bi.other_rows` (pad with spaces if short).
pub(in crate::vim) fn apply_block_insert(buffer: &mut Buffer, bi: &BlockInsert, text: &str) {
    let mut rows = bi.other_rows.clone();
    rows.sort_unstable();
    for row in rows.into_iter().rev() {
        let rope = buffer.rope();
        if row >= rope.len_lines() {
            continue;
        }
        let line_start = rope.line_to_char(row);
        let len = block::line_content_len(rope, row);
        let mut insert_at = line_start + bi.col.min(len);
        let pad = bi.col.saturating_sub(len);
        let mut chunk = String::new();
        if pad > 0 {
            chunk.extend(std::iter::repeat_n(' ', pad));
            insert_at = line_start + len;
        }
        chunk.push_str(text);
        buffer.set_cursor_raw(insert_at);
        buffer.clear_selection();
        buffer.replace_selection(&chunk);
    }
}
