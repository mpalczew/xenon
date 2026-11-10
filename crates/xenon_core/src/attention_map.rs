use std::collections::HashMap;

use crate::attention::AttentionReason;
use crate::{TabId, WorkspaceId};

#[derive(Clone, Debug, Default)]
pub struct AttentionMap {
    marks: HashMap<(WorkspaceId, TabId), AttentionReason>,
}

impl AttentionMap {
    pub fn flag(&mut self, workspace: WorkspaceId, tab: TabId, reason: AttentionReason) -> bool {
        !matches!(self.marks.insert((workspace, tab), reason), Some(prev) if prev == reason)
    }

    pub fn clear_tab(&mut self, workspace: WorkspaceId, tab: TabId) -> bool {
        self.marks.remove(&(workspace, tab)).is_some()
    }

    pub fn clear_workspace(&mut self, workspace: WorkspaceId) -> bool {
        let before = self.marks.len();
        self.marks.retain(|(ws, _), _| *ws != workspace);
        self.marks.len() != before
    }

    pub fn reason(&self, workspace: WorkspaceId, tab: TabId) -> Option<AttentionReason> {
        self.marks.get(&(workspace, tab)).copied()
    }
}
