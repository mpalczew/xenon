//! Looking things up by caret or click position.

use std::ops::Range;

use crate::{Checkbox, Doc, Kind, LinkRef, Markup};

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

    /// The bullets that lie inside `range`.
    pub fn bullets_in(&self, range: &Range<usize>) -> &[Markup] {
        let a = self.bullets.partition_point(|m| m.range.end <= range.start);
        let b = self.bullets.partition_point(|m| m.range.start < range.end);
        &self.bullets[a..b.max(a)]
    }

    /// Does a list item start on the line that begins at `line_start`?
    pub fn is_item_line(&self, line_start: usize) -> bool {
        self.spans
            .iter()
            .any(|s| matches!(s.kind, Kind::ListItem(_)) && s.range.start == line_start)
    }
}
