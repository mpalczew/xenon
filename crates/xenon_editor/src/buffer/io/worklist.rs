use super::{Buffer, ExternalState, SaveError, mtime};
use anyhow::Result;
use std::path::Path;

impl Buffer {
    /// Save through the same validated and revision-checked path as capture.
    pub fn save_worklist(&mut self, root: &Path) -> std::result::Result<(), SaveError> {
        let file = crate::worklist_file::WorklistFile::new(root)
            .map_err(|error| SaveError::Io(std::io::Error::other(error)))?;
        if file
            .read()
            .map_err(|error| SaveError::Io(std::io::Error::other(error)))?
            .as_deref()
            != self.disk_snapshot.as_deref()
        {
            return Err(SaveError::ExternalChange);
        }
        let initial = self.never_created && self.rope.len_chars() == 0;
        let updated = if initial {
            b"# Worklist\n".to_vec()
        } else {
            self.rope.to_string().into_bytes()
        };
        if let Err(error) = file.write(self.disk_snapshot.as_deref(), &updated) {
            if file.read().ok().flatten().as_deref() != self.disk_snapshot.as_deref() {
                return Err(SaveError::ExternalChange);
            }
            return Err(SaveError::Io(std::io::Error::other(error)));
        }
        self.disk_snapshot = Some(updated);
        if initial {
            self.rope = ropey::Rope::from_str("# Worklist\n");
        }
        self.never_created = false;
        self.disk_mtime = mtime(&self.path);
        self.mark_clean();
        Ok(())
    }

    pub(crate) fn worklist_needs_creation(&self) -> bool {
        self.never_created
    }

    pub fn check_worklist_external(&mut self, root: &Path) -> Result<ExternalState> {
        let current = crate::worklist_file::WorklistFile::new(root)?.read()?;
        if current.is_none() {
            if self.never_created {
                return Ok(ExternalState::Unchanged);
            }
            self.mark_dirty();
            return Ok(ExternalState::Deleted);
        }
        if current == self.disk_snapshot {
            return Ok(ExternalState::Unchanged);
        }
        if self.is_dirty() {
            return Ok(ExternalState::Conflicted);
        }
        self.reload()?;
        Ok(ExternalState::Reloaded)
    }
}
