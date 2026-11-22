use xenon_core::{
    PaneId, TabId, TermId, WorkspaceDot, WorkspaceId, workspace_dot, workspace_dot_from_tabs,
};
use xenon_term::TermStatus;

use crate::model::{Model, Tab, TabKind};

impl Model {
    pub(crate) fn active_editor_key(&self) -> Option<(WorkspaceId, TabId)> {
        let ws = self.active()?;
        let tab = self.spaces.get(&ws)?.content.focused_leaf()?.active_tab()?;
        matches!(tab.kind, TabKind::Kept(xenon_core::TabState::Editor { .. }))
            .then_some((ws, tab.id))
    }

    pub(crate) fn active_editor_dirty(&self) -> bool {
        self.active_editor_key()
            .and_then(|(ws, tab)| self.tab(ws, tab))
            .is_some_and(|t| t.modified)
    }

    pub(crate) fn dirty_editor(&self, ws: WorkspaceId) -> Option<String> {
        let root = self.spaces.get(&ws)?.content.root.as_ref()?;
        let mut found = None;
        root.for_each_leaf(&mut |leaf| {
            found = found.take().or_else(|| {
                leaf.tabs
                    .iter()
                    .find(|t| t.modified)
                    .map(|tab| match &tab.kind {
                        TabKind::Kept(xenon_core::TabState::Editor { path, .. }) => path
                            .file_name()
                            .map_or_else(|| "a file".into(), |n| n.to_string_lossy().into_owned()),
                        _ => tab.title.clone(),
                    })
            });
        });
        found
    }

    pub(crate) fn is_viewed(&self, ws: WorkspaceId, tab: TabId) -> bool {
        self.window_active
            && self.active() == Some(ws)
            && self
                .spaces
                .get(&ws)
                .and_then(|s| s.content.active_tab())
                .is_some_and(|t| t.id == tab)
    }

    pub(crate) fn apply_status(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        status: TermStatus,
    ) -> Applied {
        let mut applied = Applied::default();
        if let Some(t) = self.tab_mut(ws, tab) {
            if t.title != status.title {
                t.title = status.title.clone();
                applied.relabel = true;
            }
            if t.phone_device != status.phone {
                t.phone_device = status.phone.clone();
                applied.relabel = true;
            }
            let code = status.exit.map(|e| e.code());
            if t.exited != code {
                t.exited = code;
                applied.exited = code;
            }
        }
        applied.dots = self.tab_dot_of(Some(&status)) != self.tab_dot(ws, tab);
        self.status.insert((ws, tab), status);
        applied
    }

    pub(crate) fn dismiss_viewed(&mut self) -> Option<TermId> {
        let ws = self.active()?;
        let tab = self.spaces.get(&ws)?.content.active_tab()?;
        let (id, term) = (tab.id, tab.term);
        let status = self.status.get_mut(&(ws, id))?;
        let flagged = status.attention.take().is_some();
        (flagged && self.is_viewed(ws, id))
            .then_some(term)
            .flatten()
    }

    pub(crate) fn tab_of_term(&self, id: TermId) -> Option<(WorkspaceId, TabId)> {
        self.spaces.iter().find_map(|(ws, space)| {
            let mut found = None;
            if let Some(root) = space.content.root.as_ref() {
                root.for_each_leaf(&mut |leaf| {
                    let hit = leaf.tabs.iter().find(|t| t.term == Some(id));
                    found = found.or(hit.map(|t| t.id));
                });
            }
            found.map(|tab| (*ws, tab))
        })
    }

    pub(crate) fn focus_pane(&mut self, ws: WorkspaceId, pane: PaneId) -> bool {
        let Some(content) = self.space_content(ws) else {
            return false;
        };
        let changed = content.focused != Some(pane) && content.leaf_ids().contains(&pane);
        if changed {
            content.focused = Some(pane);
        }
        changed
    }

    pub(crate) fn tab_dot(&self, ws: WorkspaceId, tab: TabId) -> Option<WorkspaceDot> {
        self.tab_dot_of(self.status.get(&(ws, tab)))
    }

    fn tab_dot_of(&self, status: Option<&TermStatus>) -> Option<WorkspaceDot> {
        let s = status?;
        workspace_dot(s.working, s.attention)
    }

    pub(crate) fn workspace_dot(&self, ws: WorkspaceId) -> Option<WorkspaceDot> {
        let root = self.spaces.get(&ws)?.content.root.as_ref()?;
        let mut tabs = Vec::new();
        root.for_each_leaf(&mut |leaf| {
            for tab in leaf
                .tabs
                .iter()
                .filter(|t: &&Tab| matches!(t.kind, TabKind::Terminal))
            {
                let status = self.status.get(&(ws, tab.id));
                tabs.push((
                    status.is_some_and(|s| s.working),
                    status.and_then(|s| s.attention),
                ));
            }
        });
        workspace_dot_from_tabs(tabs)
    }

    pub(crate) fn any_working(&self) -> bool {
        self.status.values().any(|s| s.working)
    }
}

#[derive(Default)]
pub(crate) struct Applied {
    pub relabel: bool,
    pub dots: bool,
    pub exited: Option<i32>,
}
