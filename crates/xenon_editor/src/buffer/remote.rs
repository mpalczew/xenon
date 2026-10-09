use super::Buffer;

impl Buffer {
    pub(crate) fn from_remote(path: std::path::PathBuf, text: &str) -> Self {
        let mut buffer = Self::empty(path);
        buffer.rope = ropey::Rope::from_str(text);
        buffer.never_created = false;
        buffer
    }

    pub(crate) fn generation(&self) -> u64 {
        self.version
    }

    pub(crate) fn mutation(&self) -> u64 {
        self.mutation
    }

    pub(crate) fn remote_dirty(&self, saved: &str) -> bool {
        self.is_dirty() || self.text() != saved
    }

    pub(crate) fn remote_saved(&mut self, generation: u64, text: &str) {
        self.saved_version = generation;
        if self.text() != text {
            self.mark_dirty();
        }
    }

    pub(crate) fn remote_reload(&mut self, text: &str) {
        self.mutation = self.mutation.wrapping_add(1);
        self.rope = ropey::Rope::from_str(text);
        self.cursor = self.cursor.min(self.rope.len_chars());
        self.selection_anchor = None;
        self.version = self.version.saturating_add(1);
        self.saved_version = self.version;
        self.undo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditCommand;

    #[test]
    fn acknowledged_save_keeps_edits_made_during_the_request_dirty() {
        let mut buffer = Buffer::from_remote("/virtual/file.rs".into(), "initial");
        buffer.apply(EditCommand::Insert("first ".into()));
        let generation = buffer.generation();
        let submitted = buffer.text();
        buffer.apply(EditCommand::Insert("second ".into()));
        buffer.remote_saved(generation, &submitted);
        assert!(buffer.is_dirty());
        assert!(buffer.text().contains("second"));
    }

    #[test]
    fn acknowledged_save_marks_the_submitted_buffer_clean() {
        let mut buffer = Buffer::from_remote("/virtual/file.rs".into(), "initial");
        buffer.apply(EditCommand::Insert("draft ".into()));
        buffer.remote_saved(buffer.generation(), &buffer.text());
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn undo_branch_cannot_make_different_remote_edits_look_saved() {
        let mut buffer = Buffer::from_remote("/virtual/file.rs".into(), "initial");
        buffer.apply(EditCommand::Insert("submitted ".into()));
        let submitted = buffer.text();
        let generation = buffer.generation();
        buffer.undo();
        buffer.apply(EditCommand::Insert("other ".into()));
        buffer.remote_saved(generation, &submitted);
        buffer.undo();
        buffer.redo();
        assert!(buffer.remote_dirty(&submitted));
    }

    #[test]
    fn reload_keeps_cursor_in_bounds_and_resets_undo() {
        let mut buffer = Buffer::from_remote("/virtual/file.rs".into(), "initial");
        buffer.apply(EditCommand::Insert("draft ".into()));
        buffer.remote_reload("remote");
        assert_eq!(buffer.text(), "remote");
        assert!(!buffer.is_dirty());
        assert!(!buffer.undo());
        assert!(buffer.cursor() <= "remote".len());
    }
}
