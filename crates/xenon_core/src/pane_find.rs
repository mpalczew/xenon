//! Looking tabs up in a pane tree.

use crate::content::{PaneId, TabId};
use crate::pane_tree::{PaneLeaf, PaneTab, PaneTree};

impl<T> PaneTree<T> {
    /// First (pane, index) whose tab satisfies `matches`.
    pub fn find_position(&self, matches: &dyn Fn(&T) -> bool) -> Option<(PaneId, usize)> {
        let mut found = None;
        self.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                found = leaf.tabs.iter().position(matches).map(|i| (leaf.id, i));
            }
        });
        found
    }

    /// Leaf holding a tab that satisfies `matches`.
    pub fn find_leaf_mut_where(
        &mut self,
        matches: &dyn Fn(&T) -> bool,
    ) -> Option<&mut PaneLeaf<T>> {
        match self {
            Self::Leaf(l) => l.tabs.iter().any(matches).then_some(l),
            Self::Split { children, .. } => children
                .iter_mut()
                .find_map(|c| c.find_leaf_mut_where(matches)),
        }
    }

    /// (pane, index) of a tab.
    pub fn find_tab(&self, tab: TabId) -> Option<(PaneId, usize)>
    where
        T: PaneTab,
    {
        self.find_position(&|t: &T| t.tab_id() == tab)
    }

    pub(crate) fn max_pane_id(&self) -> u64 {
        let mut max = 0;
        self.for_each_leaf(&mut |leaf| max = max.max(leaf.id.0));
        max
    }

    pub(crate) fn max_tab_id(&self) -> u64
    where
        T: PaneTab,
    {
        let mut max = 0;
        self.for_each_leaf(&mut |leaf| {
            for tab in &leaf.tabs {
                max = max.max(tab.tab_id().0);
            }
        });
        max
    }
}
