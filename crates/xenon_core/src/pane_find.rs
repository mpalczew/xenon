use crate::content::{PaneId, TabId};
use crate::pane_tree::{PaneTab, PaneTree};

impl<T> PaneTree<T> {
    pub(crate) fn find_position(&self, matches: &dyn Fn(&T) -> bool) -> Option<(PaneId, usize)> {
        let mut found = None;
        self.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                found = leaf.tabs.iter().position(matches).map(|i| (leaf.id, i));
            }
        });
        found
    }

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
