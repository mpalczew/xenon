//! Disk open/save and external-change checks for `Buffer`.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::Result;

use super::{BINARY_SNIFF_BYTES, Buffer, ExternalState, OpenError, SaveError};
use crate::undo::UndoStack;

impl Buffer {
    /// Read `path` into a buffer, rejecting binary files.
    pub fn open(path: impl AsRef<Path>) -> std::result::Result<Buffer, OpenError> {
        let path = path.as_ref().to_path_buf();
        let bytes = fs::read(&path)?;
        if bytes.iter().take(BINARY_SNIFF_BYTES).any(|&b| b == 0) {
            return Err(OpenError::Binary(path));
        }
        let rope = ropey::Rope::from_reader(&bytes[..])?;
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
        self.write_to_disk()
    }

    /// Write even if disk mtime moved (vim `:w!`).
    pub fn save_force(&mut self) -> std::result::Result<(), SaveError> {
        self.write_to_disk()
    }

    fn write_to_disk(&mut self) -> std::result::Result<(), SaveError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(&self.path)?;
        self.rope.write_to(&mut file)?;
        self.disk_mtime = mtime(&self.path);
        self.dirty = false;
        Ok(())
    }

    /// Rebind this buffer to a new path (Save As). Does not write.
    pub fn set_path(&mut self, path: impl AsRef<Path>) {
        self.path = path.as_ref().to_path_buf();
        self.disk_mtime = None;
        self.dirty = true;
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
        self.rope = ropey::Rope::from_reader(&bytes[..])?;
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
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}
