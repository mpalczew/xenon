//! Edit commands: pure mutations of a `Buffer`'s rope and cursor. Cursor
//! positions are character indices throughout.

use crate::buffer::Buffer;
use crate::selection;
use crate::undo::Edit;

pub enum EditCommand {
    Insert(String),
    Newline,
    Backspace,
    Delete,
    Move(Motion),
    /// Move while extending the selection (shift+arrow).
    Extend(Motion),
    SetCursor(usize),
    Undo,
    Redo,
}

pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    FileStart,
    FileEnd,
    WordLeft,
    WordRight,
}

impl EditCommand {
    /// Whether this command mutates the text (vs. just moving the cursor).
    pub fn edits(&self) -> bool {
        matches!(
            self,
            EditCommand::Insert(_)
                | EditCommand::Newline
                | EditCommand::Backspace
                | EditCommand::Delete
                | EditCommand::Undo
                | EditCommand::Redo
        )
    }
}

impl Buffer {
    pub fn apply(&mut self, command: EditCommand) {
        match command {
            EditCommand::Insert(text) => self.insert(&text),
            EditCommand::Newline => self.insert("\n"),
            EditCommand::Backspace => self.backspace(),
            EditCommand::Delete => self.delete(),
            EditCommand::Move(motion) => self.move_cursor(motion, false),
            EditCommand::Extend(motion) => self.move_cursor(motion, true),
            EditCommand::SetCursor(index) => {
                self.clear_selection();
                self.set_cursor_raw(index);
            }
            EditCommand::Undo => {
                self.undo();
            }
            EditCommand::Redo => {
                self.redo();
            }
        }
    }

    fn insert(&mut self, text: &str) {
        self.replace_selection(text);
    }

    fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let cursor = self.cursor();
        if cursor == 0 {
            return;
        }
        let start = cursor - 1;
        let old = self.rope().slice(start..cursor).to_string();
        self.apply_edit(Edit {
            start,
            old,
            new: String::new(),
        });
    }

    fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let cursor = self.cursor();
        if cursor >= self.rope().len_chars() {
            return;
        }
        let old = self.rope().slice(cursor..cursor + 1).to_string();
        self.apply_edit(Edit {
            start: cursor,
            old,
            new: String::new(),
        });
    }

    fn move_cursor(&mut self, motion: Motion, extend: bool) {
        let cursor = self.cursor();
        if extend {
            if self.selection_anchor().is_none() {
                self.set_anchor_raw(Some(cursor));
            }
        } else {
            // Collapse to the edge in the direction of movement when a selection exists.
            if let Some(range) = self.selection_range() {
                let target = match motion {
                    Motion::Left
                    | Motion::Up
                    | Motion::LineStart
                    | Motion::FileStart
                    | Motion::WordLeft => range.start,
                    Motion::Right
                    | Motion::Down
                    | Motion::LineEnd
                    | Motion::FileEnd
                    | Motion::WordRight => range.end,
                };
                self.clear_selection();
                self.set_cursor_raw(target);
                // Arrow once collapses; don't also apply the motion.
                if matches!(
                    motion,
                    Motion::Left | Motion::Right | Motion::Up | Motion::Down
                ) {
                    return;
                }
            } else {
                self.clear_selection();
            }
        }
        let cursor = self.cursor();
        let target = match motion {
            Motion::Left => cursor.saturating_sub(1),
            Motion::Right => (cursor + 1).min(self.rope().len_chars()),
            Motion::LineStart => self.line_start(cursor),
            Motion::LineEnd => self.line_end(cursor),
            Motion::Up => self.vertical(cursor, -1),
            Motion::Down => self.vertical(cursor, 1),
            Motion::FileStart => 0,
            Motion::FileEnd => self.rope().len_chars(),
            Motion::WordLeft => word_left(self.rope(), cursor),
            Motion::WordRight => word_right(self.rope(), cursor),
        };
        self.set_cursor_raw(target);
    }

    fn line_start(&self, cursor: usize) -> usize {
        let row = self.rope().char_to_line(cursor);
        self.rope().line_to_char(row)
    }

    fn line_end(&self, cursor: usize) -> usize {
        let row = self.rope().char_to_line(cursor);
        self.line_start(cursor) + line_len(self, row)
    }

    /// Move `delta` rows, keeping the column where possible.
    fn vertical(&self, cursor: usize, delta: isize) -> usize {
        let rope = self.rope();
        let row = rope.char_to_line(cursor);
        let col = cursor - rope.line_to_char(row);
        let target_row = row as isize + delta;
        if target_row < 0 || target_row as usize >= rope.len_lines() {
            return cursor;
        }
        let target_row = target_row as usize;
        let clamped_col = col.min(line_len(self, target_row));
        rope.line_to_char(target_row) + clamped_col
    }
}

fn line_len(buffer: &Buffer, row: usize) -> usize {
    let line = buffer.rope().line(row);
    let len = line.len_chars();
    if len > 0 && line.char(len - 1) == '\n' {
        len - 1
    } else {
        len
    }
}

fn word_left(rope: &ropey::Rope, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    // Skip one char left into the previous run, then to its start.
    let at = cursor - 1;
    selection::word_range_at(rope, at).start
}

fn word_right(rope: &ropey::Rope, cursor: usize) -> usize {
    let len = rope.len_chars();
    if cursor >= len {
        return len;
    }
    let end = selection::word_range_at(rope, cursor).end;
    // Skip spaces after the word so ⌥→ lands on the next token.
    let mut i = end;
    while i < len && rope.char(i).is_whitespace() && rope.char(i) != '\n' {
        i += 1;
    }
    i
}
