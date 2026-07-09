//! Edit commands: pure mutations of a `Buffer`'s rope and cursor. Cursor
//! positions are character indices throughout.

use crate::buffer::Buffer;

pub enum EditCommand {
    Insert(String),
    Newline,
    Backspace,
    Delete,
    Move(Motion),
    SetCursor(usize),
}

pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
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
            EditCommand::Move(motion) => self.move_cursor(motion),
            EditCommand::SetCursor(index) => self.set_cursor_raw(index),
        }
    }

    fn insert(&mut self, text: &str) {
        let cursor = self.cursor();
        self.rope_mut().insert(cursor, text);
        self.set_cursor_raw(cursor + text.chars().count());
        self.mark_dirty();
    }

    fn backspace(&mut self) {
        let cursor = self.cursor();
        if cursor == 0 {
            return;
        }
        self.rope_mut().remove(cursor - 1..cursor);
        self.set_cursor_raw(cursor - 1);
        self.mark_dirty();
    }

    fn delete(&mut self) {
        let cursor = self.cursor();
        if cursor >= self.rope().len_chars() {
            return;
        }
        self.rope_mut().remove(cursor..cursor + 1);
        self.mark_dirty();
    }

    fn move_cursor(&mut self, motion: Motion) {
        let cursor = self.cursor();
        let target = match motion {
            Motion::Left => cursor.saturating_sub(1),
            Motion::Right => cursor + 1,
            Motion::LineStart => self.line_start(cursor),
            Motion::LineEnd => self.line_end(cursor),
            Motion::Up => self.vertical(cursor, -1),
            Motion::Down => self.vertical(cursor, 1),
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
