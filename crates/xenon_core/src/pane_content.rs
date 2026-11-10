use crate::content::{ContentLayout, PaneId, TabId, TabState};
use crate::pane_tree::{PaneLeaf, PaneTab, PaneTree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkOutcome {
    Unchanged,
    NoRoom,
    Created,
    Added(PaneId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    FocusPane(PaneId),
    Empty,
    Kept,
}

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

    pub(crate) fn next_pane_id(&self) -> PaneId {
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
