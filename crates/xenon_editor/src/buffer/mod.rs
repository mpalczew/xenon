//! `Buffer`: a rope-backed text buffer with save and external-change tracking.
//! Pure logic (no gpui) so it can be unit tested directly.

mod io;
mod sel;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ropey::Rope;

use crate::undo::{Edit, UndoStack};

/// How many leading bytes to scan for a NUL when detecting binary files.
const BINARY_SNIFF_BYTES: usize = 8000;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("{0} looks like a binary file")]
    Binary(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("file changed on disk since it was opened")]
    ExternalChange,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What a disk re-check found relative to the in-memory buffer.
#[derive(Debug, PartialEq, Eq)]
pub enum ExternalState {
    Unchanged,
    Reloaded,
    Conflicted,
    Deleted,
}

pub struct Buffer {
    rope: Rope,
    path: PathBuf,
    cursor: usize,
    /// Selection anchor; head is always `cursor`. `None` means empty selection.
    selection_anchor: Option<usize>,
    /// Content generation; bumps once per undoable transaction (or forced dirty).
    version: u64,
    /// `version` at last open/save/reload. Dirty when these differ.
    saved_version: u64,
    disk_mtime: Option<SystemTime>,
    undo: UndoStack,
}

impl Buffer {
    /// Open/close a multi-edit undo step (vim insert session, change+type, …).
    pub(crate) fn set_undo_group(&mut self, open: bool) {
        if open {
            self.undo.begin_group();
        } else {
            self.undo.end_group();
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_dirty(&self) -> bool {
        self.version != self.saved_version
    }

    pub(crate) fn rope(&self) -> &Rope {
        &self.rope
    }

    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// Cursor as a zero-based (row, column) in characters.
    pub fn cursor_position(&self) -> (usize, usize) {
        let row = self.rope.char_to_line(self.cursor);
        let col = self.cursor - self.rope.line_to_char(row);
        (row, col)
    }

    /// Move the cursor to a zero-based row/column, clamping both to the buffer.
    /// Clears the selection unless `extend` is true.
    pub fn set_cursor_position(&mut self, row: usize, col: usize) {
        self.set_cursor_position_extend(row, col, false);
    }

    pub(crate) fn set_cursor_position_extend(&mut self, row: usize, col: usize, extend: bool) {
        let last_row = self.rope.len_lines().saturating_sub(1);
        let row = row.min(last_row);
        let col = col.min(self.line_len(row));
        let cursor = self.rope.line_to_char(row) + col;
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        self.cursor = cursor;
    }

    /// Whole-buffer text (used by tests and highlighting).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Replace the selection (or insert at cursor if empty) with `text`.
    pub(crate) fn replace_selection(&mut self, text: &str) {
        let (start, old) = match self.selection_range() {
            Some(range) => {
                let old = self.rope.slice(range.clone()).to_string();
                (range.start, old)
            }
            None => (self.cursor, String::new()),
        };
        self.apply_edit(Edit {
            start,
            old,
            new: text.to_string(),
        });
        self.selection_anchor = None;
    }

    /// Replace a char range with `text` (one undo step unless grouped).
    pub(crate) fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let len = self.rope.len_chars();
        let start = range.start.min(len);
        let end = range.end.min(len).max(start);
        let old = if start < end {
            self.rope.slice(start..end).to_string()
        } else {
            String::new()
        };
        self.apply_edit(Edit {
            start,
            old,
            new: text.to_string(),
        });
        self.selection_anchor = None;
    }

    /// Delete the selection if any; returns whether anything was deleted.
    pub(crate) fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            return false;
        };
        let old = self.rope.slice(range.clone()).to_string();
        self.apply_edit(Edit {
            start: range.start,
            old,
            new: String::new(),
        });
        self.selection_anchor = None;
        true
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some((edits, version_before)) = self.undo.undo() else {
            return false;
        };
        for edit in edits {
            self.apply_raw(&edit.new, &edit.old, edit.start);
        }
        self.version = version_before;
        self.selection_anchor = None;
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some((edits, version_after)) = self.undo.redo() else {
            return false;
        };
        for edit in edits {
            self.apply_raw(&edit.old, &edit.new, edit.start);
        }
        self.version = version_after;
        self.selection_anchor = None;
        true
    }

    pub(crate) fn apply_edit(&mut self, edit: Edit) {
        if edit.old.is_empty() && edit.new.is_empty() {
            return;
        }
        // One version bump per undo step: first edit in an open group, or each
        // ungrouped edit. Later keystrokes in the same insert session share it.
        let version_before = if self.undo.group_has_edits() {
            self.version.saturating_sub(1)
        } else {
            let before = self.version;
            self.version = self.version.saturating_add(1);
            before
        };
        self.apply_raw(&edit.old, &edit.new, edit.start);
        self.undo.push(edit, version_before);
    }

    fn apply_raw(&mut self, old: &str, new: &str, start: usize) {
        let old_len = old.chars().count();
        let end = start + old_len;
        if old_len > 0 {
            self.rope.remove(start..end.min(self.rope.len_chars()));
        }
        if !new.is_empty() {
            self.rope.insert(start, new);
        }
        self.cursor = start + new.chars().count();
    }

    /// Mark dirty without a content change (new path, deleted file on disk).
    fn mark_dirty(&mut self) {
        if self.version == self.saved_version {
            self.version = self.version.saturating_add(1);
        }
    }

    fn mark_clean(&mut self) {
        self.saved_version = self.version;
    }

    pub(crate) fn set_cursor_raw(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.rope.len_chars());
    }

    pub(crate) fn set_anchor_raw(&mut self, anchor: Option<usize>) {
        self.selection_anchor = anchor.map(|a| a.min(self.rope.len_chars()));
    }

    fn line_len(&self, row: usize) -> usize {
        let line = self.rope.line(row);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }
}
