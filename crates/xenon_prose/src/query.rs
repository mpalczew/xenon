//! Looking things up by caret or click position.

use crate::{Checkbox, Doc, LinkRef};

impl Doc {
    /// The link whose text contains `pos`.
    pub fn link_at(&self, pos: usize) -> Option<&LinkRef> {
        self.links
            .iter()
            .find(|l| l.range.start <= pos && pos < l.range.end)
    }

    /// The task box whose `[ ]` contains `pos`.
    pub fn checkbox_at(&self, pos: usize) -> Option<&Checkbox> {
        self.checkboxes
            .iter()
            .find(|c| c.range.start <= pos && pos < c.range.end)
    }

    /// The task box on the line the caret is on.
    pub fn checkbox_on_line_of(&self, pos: usize) -> Option<&Checkbox> {
        self.checkboxes
            .iter()
            .find(|c| c.line.start <= pos && pos <= c.line.end)
    }
}
