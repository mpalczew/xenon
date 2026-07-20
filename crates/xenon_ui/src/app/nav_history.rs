//! Browser-style back/forward stack for content tabs (⌘[ / ⌘]).
//!
//! Per-workspace, in-memory, keyed by `TabId`. XenonApp wiring lives in
//! `keyboard.rs` (seed / visit / go_back / go_forward).

use xenon_core::TabId;

const MAX_ENTRIES: usize = 50;

/// Back/forward stack for one workspace.
#[derive(Debug, Default, Clone)]
pub(crate) struct NavHistory {
    stack: Vec<TabId>,
    /// Index of the current location in `stack` (0 when empty).
    index: usize,
}

impl NavHistory {
    pub(crate) fn current(&self) -> Option<TabId> {
        self.stack.get(self.index).copied()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Record a user-driven visit. Truncates any forward branch (browser model).
    pub(crate) fn visit(&mut self, tab: TabId) {
        if self.current() == Some(tab) {
            return;
        }
        if !self.stack.is_empty() {
            self.stack.truncate(self.index + 1);
        }
        self.stack.push(tab);
        self.index = self.stack.len() - 1;
        self.cap();
    }

    /// Seed history when the stack is empty (e.g. first workspace activate).
    pub(crate) fn seed_if_empty(&mut self, tab: TabId) {
        if self.stack.is_empty() {
            self.stack.push(tab);
            self.index = 0;
        }
    }

    pub(crate) fn can_back(&self) -> bool {
        self.index > 0
    }

    pub(crate) fn can_forward(&self) -> bool {
        !self.stack.is_empty() && self.index + 1 < self.stack.len()
    }

    /// Move one step back. Returns the new current tab.
    pub(crate) fn back(&mut self) -> Option<TabId> {
        if !self.can_back() {
            return None;
        }
        self.index -= 1;
        self.current()
    }

    /// Move one step forward. Returns the new current tab.
    pub(crate) fn forward(&mut self) -> Option<TabId> {
        if !self.can_forward() {
            return None;
        }
        self.index += 1;
        self.current()
    }

    /// Drop every entry matching `dead` and repair `index`.
    pub(crate) fn prune(&mut self, dead: TabId) {
        self.retain(|t| t != dead);
    }

    /// Keep only tabs for which `keep` returns true.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(TabId) -> bool) {
        if self.stack.is_empty() {
            return;
        }
        let old_index = self.index;
        let mut new_stack = Vec::with_capacity(self.stack.len());
        let mut new_index = 0usize;
        for (i, &t) in self.stack.iter().enumerate() {
            if !keep(t) {
                continue;
            }
            if i <= old_index {
                new_index = new_stack.len();
            }
            new_stack.push(t);
        }
        if new_stack.is_empty() {
            self.stack.clear();
            self.index = 0;
            return;
        }
        self.stack = new_stack;
        self.index = new_index.min(self.stack.len() - 1);
    }

    fn cap(&mut self) {
        if self.stack.len() <= MAX_ENTRIES {
            return;
        }
        let drop_n = self.stack.len() - MAX_ENTRIES;
        self.stack.drain(0..drop_n);
        self.index = self.index.saturating_sub(drop_n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(n: u64) -> TabId {
        TabId(n)
    }

    #[test]
    fn visit_builds_stack_and_truncates_forward() {
        let mut h = NavHistory::default();
        h.visit(t(1));
        h.visit(t(2));
        h.visit(t(3));
        assert_eq!(h.current(), Some(t(3)));
        assert!(h.can_back());
        assert!(!h.can_forward());

        assert_eq!(h.back(), Some(t(2)));
        assert_eq!(h.back(), Some(t(1)));
        assert!(!h.can_back());
        assert!(h.can_forward());

        // Branch from middle: drop forward.
        h.visit(t(9));
        assert_eq!(h.current(), Some(t(9)));
        assert!(!h.can_forward());
        assert_eq!(h.back(), Some(t(1)));
    }

    #[test]
    fn visit_same_is_noop() {
        let mut h = NavHistory::default();
        h.visit(t(1));
        h.visit(t(1));
        assert_eq!(h.stack.len(), 1);
    }

    #[test]
    fn prune_repairs_index() {
        let mut h = NavHistory::default();
        h.visit(t(1));
        h.visit(t(2));
        h.visit(t(3));
        h.back(); // on 2
        h.prune(t(2));
        assert_eq!(h.stack, vec![t(1), t(3)]);
        assert_eq!(h.current(), Some(t(1)));
    }

    #[test]
    fn seed_if_empty() {
        let mut h = NavHistory::default();
        h.seed_if_empty(t(1));
        h.seed_if_empty(t(2));
        assert_eq!(h.current(), Some(t(1)));
        assert_eq!(h.stack.len(), 1);
    }

    #[test]
    fn back_forward_empty() {
        let mut h = NavHistory::default();
        assert!(h.back().is_none());
        assert!(h.forward().is_none());
        h.visit(t(1));
        assert!(h.back().is_none());
        assert!(h.forward().is_none());
    }
}
