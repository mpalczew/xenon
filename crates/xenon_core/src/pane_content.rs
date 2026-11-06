//! One workspace's pane tree plus the focused leaf, and the moves on it.

use crate::content::{ContentLayout, PaneId, TabId, TabState};
use crate::pane_tree::{PaneLeaf, PaneTab, PaneTree};

/// What [`PaneContent::park_empty`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkOutcome {
    /// Nothing to split (no focused pane, or it is missing).
    Unchanged,
    /// The split would exceed the depth or fan-out limit.
    NoRoom,
    /// The tree was empty; two panes were created and the first is focused.
    Created,
    /// A parked pane was added beside this (still focused) pane.
    Added(PaneId),
}

/// What closing a tab did to the layout; see [`PaneContent::close_tab`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    /// The pane stays and shows the tab the user was looking at: focus it.
    FocusPane(PaneId),
    /// The emptied pane was removed; `focused` now names the pane to focus.
    Unsplit,
    /// That was the last tab in the last pane: the tree is gone.
    Empty,
    /// Nothing visible changed.
    Kept,
}

/// One workspace's pane tree plus the focused leaf.
#[derive(Clone)]
pub struct PaneContent<T> {
    pub root: Option<PaneTree<T>>,
    pub focused: Option<PaneId>,
}

impl<T> Default for PaneContent<T> {
    fn default() -> Self {
        Self {
            root: None,
            focused: None,
        }
    }
}

impl<T> PaneContent<T> {
    pub fn focused_leaf(&self) -> Option<&PaneLeaf<T>> {
        let id = self.focused?;
        self.root.as_ref()?.find_leaf(id)
    }

    pub fn focused_leaf_mut(&mut self) -> Option<&mut PaneLeaf<T>> {
        let id = self.focused?;
        self.root.as_mut()?.find_leaf_mut(id)
    }

    pub fn active_tab(&self) -> Option<&T> {
        self.focused_leaf()?.active_tab()
    }

    pub fn next_pane_id(&self) -> PaneId {
        PaneId(self.root.as_ref().map(|r| r.max_pane_id()).unwrap_or(0) + 1)
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        self.root
            .as_ref()
            .map(PaneTree::leaf_ids)
            .unwrap_or_default()
    }

    pub fn snapshot(&self, keep: &dyn Fn(&T) -> Option<TabState>) -> ContentLayout {
        ContentLayout {
            root: self.root.as_ref().map(|root| root.snapshot(keep)),
            focused: self.focused,
        }
    }
}

impl<T: PaneTab> PaneContent<T> {
    pub fn next_tab_id(&self) -> TabId {
        let max = self.root.as_ref().map(|r| r.max_tab_id()).unwrap_or(0);
        TabId(max + 1)
    }
}
