//! Tabs that run a task.

use xenon_core::{TabId, WorkspaceId};

use crate::model::{Model, Tab};

impl Model {
    /// Add a terminal that will run task `label`. Without `select` the pane
    /// keeps showing the tab it had.
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

    /// Ready an exited task tab for a new process of task `label`.
    pub(crate) fn recycle_terminal(&mut self, ws: WorkspaceId, tab: TabId, label: &str) {
        if let Some(t) = self.tab_mut(ws, tab) {
            t.exited = None;
            t.title = label.to_string();
        }
        self.activity.remove(&(ws, tab));
        self.attention.clear_tab(ws, tab);
    }
}
