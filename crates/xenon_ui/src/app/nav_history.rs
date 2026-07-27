//! Browser-style back/forward stack for terminal and editor locations.

use std::path::PathBuf;

use xenon_core::TabId;

const MAX_ENTRIES: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NavEntry {
    Editor { path: PathBuf, row: u32, col: u32 },
    Terminal { tab: TabId },
}

#[derive(Debug, Default, Clone)]
pub(crate) struct NavHistory {
    stack: Vec<NavEntry>,
    index: usize,
}

impl NavHistory {
    pub(crate) fn current(&self) -> Option<&NavEntry> {
        self.stack.get(self.index)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub(crate) fn visit(&mut self, entry: NavEntry) {
        if self.current() == Some(&entry) {
            return;
        }
        if !self.stack.is_empty() {
            self.stack.truncate(self.index + 1);
        }
        self.stack.push(entry);
        self.index = self.stack.len() - 1;
        self.cap();
    }

    pub(crate) fn refresh_current(&mut self, entry: NavEntry) {
        if let Some(current) = self.stack.get_mut(self.index) {
            *current = entry;
        } else {
            self.visit(entry);
        }
    }

    pub(crate) fn seed_if_empty(&mut self, entry: NavEntry) {
        if self.stack.is_empty() {
            self.stack.push(entry);
            self.index = 0;
        }
    }

    pub(crate) fn back(&mut self) -> Option<NavEntry> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.current().cloned()
    }

    pub(crate) fn forward(&mut self) -> Option<NavEntry> {
        if self.stack.is_empty() || self.index + 1 >= self.stack.len() {
            return None;
        }
        self.index += 1;
        self.current().cloned()
    }

    pub(crate) fn prune_terminal(&mut self, dead: TabId) {
        self.retain(|entry| !matches!(entry, NavEntry::Terminal { tab } if *tab == dead));
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&NavEntry) -> bool) {
        if self.stack.is_empty() {
            return;
        }
        let old_index = self.index;
        let mut new_stack = Vec::with_capacity(self.stack.len());
        let mut new_index = 0;
        for (index, entry) in self.stack.drain(..).enumerate() {
            if !keep(&entry) {
                continue;
            }
            if index <= old_index {
                new_index = new_stack.len();
            }
            new_stack.push(entry);
        }
        self.stack = new_stack;
        self.index = new_index.min(self.stack.len().saturating_sub(1));
    }

    fn cap(&mut self) {
        if self.stack.len() <= MAX_ENTRIES {
            return;
        }
        let count = self.stack.len() - MAX_ENTRIES;
        self.stack.drain(0..count);
        self.index = self.index.saturating_sub(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(name: &str, col: u32) -> NavEntry {
        NavEntry::Editor {
            path: name.into(),
            row: 0,
            col,
        }
    }

    #[test]
    fn definition_transition_can_go_back_and_forward() {
        let mut history = NavHistory::default();
        history.visit(editor("a.rs", 3));
        history.visit(editor("b.rs", 7));
        assert_eq!(history.back(), Some(editor("a.rs", 3)));
        assert_eq!(history.forward(), Some(editor("b.rs", 7)));
    }

    #[test]
    fn branch_truncates_forward_entries() {
        let mut history = NavHistory::default();
        history.visit(editor("a", 0));
        history.visit(editor("b", 0));
        history.back();
        history.visit(editor("c", 0));
        assert!(history.forward().is_none());
        assert_eq!(history.back(), Some(editor("a", 0)));
    }

    #[test]
    fn refresh_snapshots_cursor_without_adding_visit() {
        let mut history = NavHistory::default();
        history.visit(editor("a", 0));
        history.refresh_current(editor("a", 8));
        history.visit(editor("b", 0));
        assert_eq!(history.back(), Some(editor("a", 8)));
    }
}
