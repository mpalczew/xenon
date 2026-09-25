//! Disk open/save and external-change checks for `Buffer`.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::Result;

use super::{BINARY_SNIFF_BYTES, Buffer, ExternalState, OpenError, SaveError};
use crate::undo::UndoStack;
mod worklist;

impl Buffer {
    /// Create an empty, unsaved buffer for a lazily created document.
    pub fn empty(path: impl AsRef<Path>) -> Buffer {
        Buffer {
            rope: ropey::Rope::new(),
            path: path.as_ref().to_path_buf(),
            cursor: 0,
            selection_anchor: None,
            version: 0,
            saved_version: 0,
            disk_mtime: None,
            disk_snapshot: None,
            never_created: true,
            undo: UndoStack::default(),
        }
    }

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
            version: 0,
            saved_version: 0,
            disk_mtime,
            disk_snapshot: Some(bytes),
            never_created: false,
            undo: UndoStack::default(),
        })
    }

    /// Write the buffer to disk. Refuses when the file changed under a dirty
    /// buffer; the caller resolves the conflict and retries.
    pub fn save(&mut self) -> std::result::Result<(), SaveError> {
        if self.is_dirty() && mtime(&self.path) != self.disk_mtime {
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
        self.mark_clean();
        Ok(())
    }

    /// Rebind this buffer to a new path (Save As). Does not write.
    pub fn set_path(&mut self, path: impl AsRef<Path>) {
        self.path = path.as_ref().to_path_buf();
        self.disk_mtime = None;
        self.mark_dirty();
    }

    /// Compare the buffer against the file on disk (called on editor focus).
    /// A clean buffer silently reloads; a dirty one reports a conflict.
    pub fn check_external(&mut self) -> Result<ExternalState> {
        let current = mtime(&self.path);
        if current.is_none() {
            if self.never_created {
                return Ok(ExternalState::Unchanged);
            }
            self.disk_mtime = None;
            self.mark_dirty();
            return Ok(ExternalState::Deleted);
        }
        if current == self.disk_mtime {
            return Ok(ExternalState::Unchanged);
        }
        if self.is_dirty() {
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
        self.disk_snapshot = Some(bytes);
        self.never_created = false;
        self.version = 0;
        self.saved_version = 0;
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

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{Buffer, EditCommand, SaveError};

    #[test]
    fn empty_buffer_stays_virtual_until_atomic_save() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".xenon/worklist.md");
        let mut buffer = Buffer::empty(&path);
        assert!(!path.exists());
        assert_eq!(
            buffer.check_worklist_external(root.path()).unwrap(),
            crate::ExternalState::Unchanged
        );
        assert!(!buffer.is_dirty());
        assert!(buffer.worklist_needs_creation());
        buffer.save_worklist(root.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Worklist\n");
        assert_eq!(buffer.text(), "# Worklist\n");
        assert!(!buffer.worklist_needs_creation());
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn virtual_worklist_loads_an_agent_created_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".xenon/worklist.md");
        let mut buffer = Buffer::empty(&path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "agent task\n").unwrap();
        assert_eq!(
            buffer.check_worklist_external(root.path()).unwrap(),
            crate::ExternalState::Reloaded
        );
        assert_eq!(buffer.text(), "agent task\n");
    }

    #[test]
    fn atomic_save_refuses_external_changes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".xenon/worklist.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "original\n").unwrap();
        let mut buffer = Buffer::open(&path).unwrap();
        buffer.apply(EditCommand::Insert("mine ".into()));
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&path, "theirs\n").unwrap();
        assert!(matches!(
            buffer.save_worklist(root.path()),
            Err(SaveError::ExternalChange)
        ));
        assert_eq!(fs::read_to_string(path).unwrap(), "theirs\n");
    }
}
