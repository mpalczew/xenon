//! Selection helpers on Buffer.

use std::ops::Range;

use super::Buffer;
use crate::selection;

impl Buffer {
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
}
