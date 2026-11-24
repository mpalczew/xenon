use xenon_core::{TabId, TermId, WorkspaceId};

use crate::model::{Model, Tab};

impl Model {
    fn push_background_tab(
        &mut self,
        ws: WorkspaceId,
        select: bool,
        make: impl FnOnce(TabId) -> Tab,
    ) -> Option<TabId> {
        let previous = self.space_content(ws)?.focused_leaf().map(|l| l.active);
        let id = self.push_tab(ws, make)?;
        if let (false, Some(previous)) = (select, previous)
            && let Some(leaf) = self.space_content(ws)?.focused_leaf_mut()
        {
            leaf.active = previous.min(leaf.tabs.len() - 1);
        }
        Some(id)
    }

    pub(crate) fn new_task_terminal(
        &mut self,
        ws: WorkspaceId,
        label: &str,
        select: bool,
    ) -> Option<TabId> {
        self.push_background_tab(ws, select, |id| {
            let mut tab = Tab::terminal(id);
            tab.title = label.to_string();
            tab
        })
    }

    pub(crate) fn adopt_terminal(&mut self, ws: WorkspaceId, term: TermId) -> Option<TabId> {
        self.push_background_tab(ws, false, |id| {
            let mut tab = Tab::terminal(id);
            tab.term = Some(term);
            tab
        })
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
