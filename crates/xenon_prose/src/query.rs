use std::ops::Range;

use crate::{Checkbox, Doc, Kind, LinkRef, Markup};

impl Doc {
    pub fn link_at(&self, pos: usize) -> Option<&LinkRef> {
        self.links
            .iter()
            .find(|l| l.range.start <= pos && pos < l.range.end)
    }

    pub fn checkbox_at(&self, pos: usize) -> Option<&Checkbox> {
        self.checkboxes
            .iter()
            .find(|c| c.range.start <= pos && pos < c.range.end)
    }

    pub fn checkbox_on_line_of(&self, pos: usize) -> Option<&Checkbox> {
        self.checkboxes
            .iter()
            .find(|c| c.line.start <= pos && pos <= c.line.end)
    }

    pub fn bullets_in(&self, range: &Range<usize>) -> &[Markup] {
        let a = self.bullets.partition_point(|m| m.range.end <= range.start);
        let b = self.bullets.partition_point(|m| m.range.start < range.end);
        &self.bullets[a..b.max(a)]
    }

    pub fn is_item_line(&self, line_start: usize) -> bool {
        self.spans
            .iter()
            .any(|s| matches!(s.kind, Kind::ListItem(_)) && s.range.start == line_start)
    }
}
