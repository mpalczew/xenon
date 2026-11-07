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
