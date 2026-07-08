//! `Buffer`: a rope-backed text buffer with save and external-change tracking.
//! Pure logic (no gpui) so it can be unit tested directly.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use ropey::Rope;

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
    dirty: bool,
    disk_mtime: Option<SystemTime>,
}

impl Buffer {
    /// Read `path` into a buffer, rejecting binary files.
    pub fn open(path: impl AsRef<Path>) -> std::result::Result<Buffer, OpenError> {
        let path = path.as_ref().to_path_buf();
        let bytes = fs::read(&path)?;
        if is_binary(&bytes) {
            return Err(OpenError::Binary(path));
        }
        let rope = Rope::from_reader(&bytes[..])?;
        let disk_mtime = mtime(&path);
        Ok(Buffer {
            rope,
            path,
            cursor: 0,
            dirty: false,
            disk_mtime,
        })
    }

    /// Write the buffer to disk. Refuses when the file changed under a dirty
    /// buffer; the caller resolves the conflict and retries.
    pub fn save(&mut self) -> std::result::Result<(), SaveError> {
        if self.dirty && self.disk_moved() {
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

    fn reload(&mut self) -> Result<()> {
        let bytes = fs::read(&self.path)?;
        self.rope = Rope::from_reader(&bytes[..])?;
        self.cursor = self.cursor.min(self.rope.len_chars());
        self.disk_mtime = mtime(&self.path);
        self.dirty = false;
        Ok(())
    }

    fn disk_moved(&self) -> bool {
        mtime(&self.path) != self.disk_mtime
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

    /// Whole-buffer text (used by tests and highlighting).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    // Mutators used by the edit commands live in `edit.rs`; these give it
    // controlled access without exposing the fields publicly.
    pub(crate) fn rope_mut(&mut self) -> &mut Rope {
        &mut self.rope
    }

    pub(crate) fn set_cursor_raw(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.rope.len_chars());
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_BYTES).any(|&b| b == 0)
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}
