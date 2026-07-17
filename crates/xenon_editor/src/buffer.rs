//! `Buffer`: a rope-backed text buffer with save and external-change tracking.
//! Pure logic (no gpui) so it can be unit tested directly.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use ropey::Rope;

use crate::selection;
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
    dirty: bool,
    disk_mtime: Option<SystemTime>,
    undo: UndoStack,
}

impl Buffer {
    /// Read `path` into a buffer, rejecting binary files.
    pub fn open(path: impl AsRef<Path>) -> std::result::Result<Buffer, OpenError> {
        let path = path.as_ref().to_path_buf();
        let bytes = fs::read(&path)?;
        if bytes.iter().take(BINARY_SNIFF_BYTES).any(|&b| b == 0) {
            return Err(OpenError::Binary(path));
        }
        let rope = Rope::from_reader(&bytes[..])?;
        let disk_mtime = mtime(&path);
        Ok(Buffer {
            rope,
            path,
            cursor: 0,
            selection_anchor: None,
            dirty: false,
            disk_mtime,
            undo: UndoStack::default(),
        })
    }

    /// Write the buffer to disk. Refuses when the file changed under a dirty
    /// buffer; the caller resolves the conflict and retries.
    pub fn save(&mut self) -> std::result::Result<(), SaveError> {
        if self.dirty && mtime(&self.path) != self.disk_mtime {
            return Err(SaveError::ExternalChange);
        }
        let mut file = fs::File::create(&self.path)?;
        self.rope.write_to(&mut file)?;
        self.disk_mtime = mtime(&self.path);
        self.dirty = false;
        Ok(())
    }

    /// Compare the buffer against the file on disk (called on editor focus).
    /// A clean buffer silently reloads; a dirty one reports a conflict.
    pub fn check_external(&mut self) -> Result<ExternalState> {
        let current = mtime(&self.path);
        if current.is_none() {
            self.disk_mtime = None;
            self.dirty = true;
            return Ok(ExternalState::Deleted);
        }
        if current == self.disk_mtime {
            return Ok(ExternalState::Unchanged);
        }
        if self.dirty {
            return Ok(ExternalState::Conflicted);
        }
        self.reload()?;
        Ok(ExternalState::Reloaded)
    }

    /// Reload from disk (clean or after user chose disk in a conflict).
    pub(crate) fn reload(&mut self) -> Result<()> {
        let bytes = fs::read(&self.path)?;
        self.rope = Rope::from_reader(&bytes[..])?;
        self.cursor = self.cursor.min(self.rope.len_chars());
        self.selection_anchor = None;
        self.disk_mtime = mtime(&self.path);
        self.dirty = false;
        self.undo.clear();
        Ok(())
    }

    /// Treat current disk mtime as known without reloading (keep local edits).
    pub(crate) fn adopt_disk_mtime(&mut self) {
        self.disk_mtime = mtime(&self.path);
    }

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
        self.dirty
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn cursor(&self) -> usize {
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

    pub(crate) fn selection_anchor(&self) -> Option<usize> {
        self.selection_anchor
    }

    /// Active selection as a half-open char range, if non-empty.
    pub(crate) fn selection_range(&self) -> Option<Range<usize>> {
        selection::range(self.selection_anchor, self.cursor)
    }

    /// Selected text, or empty string if none.
    pub(crate) fn selected_text(&self) -> String {
        match self.selection_range() {
            Some(range) => self.rope.slice(range).to_string(),
            None => String::new(),
        }
    }

    /// Set selection explicitly (anchor + head/cursor).
    pub(crate) fn set_selection(&mut self, anchor: usize, cursor: usize) {
        let len = self.rope.len_chars();
        self.selection_anchor = Some(anchor.min(len));
        self.cursor = cursor.min(len);
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Select the word under the cursor (or at `offset` if provided).
    pub(crate) fn select_word_at(&mut self, offset: usize) {
        let range = selection::word_range_at(&self.rope, offset);
        self.selection_anchor = Some(range.start);
        self.cursor = range.end;
    }

    /// Select the line under the cursor (content only, no newline).
    pub(crate) fn select_line_at(&mut self, offset: usize) {
        let range = selection::line_range_at(&self.rope, offset);
        self.selection_anchor = Some(range.start);
        self.cursor = range.end;
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
        let Some(edits) = self.undo.undo() else {
            return false;
        };
        for edit in edits {
            self.apply_raw(&edit.new, &edit.old, edit.start);
        }
        self.selection_anchor = None;
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(edits) = self.undo.redo() else {
            return false;
        };
        for edit in edits {
            self.apply_raw(&edit.old, &edit.new, edit.start);
        }
        self.selection_anchor = None;
        true
    }

    pub(crate) fn apply_edit(&mut self, edit: Edit) {
        self.apply_raw(&edit.old, &edit.new, edit.start);
        self.undo.push(edit);
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
        self.dirty = true;
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

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}
