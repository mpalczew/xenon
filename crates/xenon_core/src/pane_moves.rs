//! Moving and closing tabs in a workspace's pane content.

use crate::content::{PaneId, TabId};
use crate::pane_content::{CloseOutcome, PaneContent};
use crate::pane_tree::{PaneTab, PaneTree};

/// Keep `active` pointing at a sensible tab after `removed` left a stack of `len`.
fn fix_active_idx(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}

impl<T> PaneContent<T> {
    pub fn unsplit_empty(&mut self, empty: PaneId) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        if matches!(root, PaneTree::Leaf(l) if l.id == empty) {
            self.root = None;
            self.focused = None;
            return;
        }
        if let Some(focus) = root.remove_leaf(empty) {
            root.normalize();
            self.focused = Some(focus);
        }
    }

    /// Remove an empty pane that has company. False when the pane has tabs or
    /// is the only one.
    pub fn remove_empty_leaf(&mut self, pane: PaneId) -> bool {
        let removable = self
            .root
            .as_ref()
            .and_then(|root| root.find_leaf(pane))
            .is_some_and(|leaf| leaf.tabs.is_empty() && self.leaf_ids().len() > 1);
        if removable {
            self.unsplit_empty(pane);
        }
        removable
    }

    /// Take the tab at (`pane`, `index`) out of its stack.
    pub fn take_tab_at(&mut self, pane: PaneId, index: usize) -> Option<T> {
        let leaf = self.root.as_mut()?.find_leaf_mut(pane)?;
        if index >= leaf.tabs.len() {
            return None;
        }
        let tab = leaf.tabs.remove(index);
        if !leaf.tabs.is_empty() {
            fix_active_idx(&mut leaf.active, index, leaf.tabs.len());
        }
        Some(tab)
    }

    /// Put a tab back as the only tab of an emptied pane.
    pub fn put_back(&mut self, pane: PaneId, tab: T) {
        if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
            leaf.tabs.push(tab);
            leaf.active = 0;
        }
    }
}

impl<T: PaneTab> PaneContent<T> {
    /// (pane, index) of a tab.
    pub fn find_tab(&self, tab: TabId) -> Option<(PaneId, usize)> {
        self.root.as_ref()?.find_tab(tab)
    }

    /// Close a tab. The pane stays, even when this empties it; only closing
    /// the pane removes it. The last pane is the exception: emptying it clears
    /// the workspace. Focus follows the closed tab only when the user was
    /// looking at it. `None` if no such tab.
    pub fn close_tab(&mut self, tab: TabId) -> Option<CloseOutcome> {
        let focused_before = self.focused;
        let root = self.root.as_mut()?;
        let (pane, idx) = root.find_tab(tab)?;
        let was_showing = focused_before == Some(pane)
            && root
                .find_leaf(pane)
                .is_some_and(|leaf| leaf.active_tab().is_some_and(|a| a.tab_id() == tab));
        let leaf = root.find_leaf_mut(pane)?;
        leaf.tabs.remove(idx);
        if !leaf.tabs.is_empty() {
            fix_active_idx(&mut leaf.active, idx, leaf.tabs.len());
        } else if self.leaf_ids().len() <= 1 {
            self.root = None;
            self.focused = None;
            return Some(CloseOutcome::Empty);
        }
        if was_showing {
            self.focused = Some(pane);
            Some(CloseOutcome::FocusPane(pane))
        } else {
            Some(CloseOutcome::Kept)
        }
    }

    /// Move `tab` into `dest` at `insert_at` (`usize::MAX` = the end). The
    /// source pane is removed if that empties it, and `dest` takes focus.
    /// False when the tab or destination is missing.
    pub fn move_tab(&mut self, tab: TabId, dest: PaneId, mut insert_at: usize) -> bool {
        let Some((src_pane, src_idx)) = self.find_tab(tab) else {
            return false;
        };
        if self.root.as_ref().and_then(|r| r.find_leaf(dest)).is_none() {
            return false;
        }
        if src_pane == dest && insert_at != usize::MAX && src_idx < insert_at {
            insert_at -= 1;
        }
        let Some(moved) = self.take_tab_at(src_pane, src_idx) else {
            return false;
        };
        if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(dest)) {
            leaf.parked = false;
            let at = if insert_at == usize::MAX {
                leaf.tabs.len()
            } else {
                insert_at.min(leaf.tabs.len())
            };
            leaf.tabs.insert(at, moved);
            leaf.active = at;
        }
        if self
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src_pane))
            .is_some_and(|l| l.tabs.is_empty())
        {
            self.unsplit_empty(src_pane);
        }
        self.focused = Some(dest);
        true
    }
}
