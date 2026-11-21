use xenon_core::{TabId, TermId, WorkspaceId};

use crate::model::{Model, Tab};

impl Model {
    pub(crate) fn new_task_terminal(
        &mut self,
        ws: WorkspaceId,
        label: &str,
        select: bool,
    ) -> Option<TabId> {
        let previous = self.space_content(ws)?.focused_leaf().map(|l| l.active);
        let id = self.push_tab(ws, |id| {
            let mut tab = Tab::terminal(id);
            tab.title = label.to_string();
            tab
        })?;
        if let (false, Some(previous)) = (select, previous)
            && let Some(leaf) = self.space_content(ws)?.focused_leaf_mut()
        {
            leaf.active = previous.min(leaf.tabs.len() - 1);
        }
        Some(id)
    }

    /// A terminal the service holds that no tab shows: a new tab for it, not selected.
    pub(crate) fn adopt_terminal(&mut self, ws: WorkspaceId, term: TermId) -> Option<TabId> {
        let previous = self.space_content(ws)?.focused_leaf().map(|l| l.active);
        let id = self.push_tab(ws, |id| {
            let mut tab = Tab::terminal(id);
            tab.term = Some(term);
            tab
        })?;
        if let Some(previous) = previous
            && let Some(leaf) = self.space_content(ws)?.focused_leaf_mut()
        {
            leaf.active = previous.min(leaf.tabs.len() - 1);
        }
        Some(id)
    }

    pub(crate) fn recycle_terminal(&mut self, ws: WorkspaceId, tab: TabId, label: &str) {
        if let Some(t) = self.tab_mut(ws, tab) {
            t.exited = None;
            t.term = None;
            t.title = label.to_string();
        }
        self.status.remove(&(ws, tab));
    }
}
