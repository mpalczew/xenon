use xenon_core::{
    AttentionReason, PaneId, TabId, WorkspaceDot, WorkspaceId, clean_terminal_title, workspace_dot,
    workspace_dot_from_tabs,
};

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

    pub(crate) fn is_viewed(&self, ws: WorkspaceId, tab: TabId) -> bool {
        self.window_active
            && self.active() == Some(ws)
            && self
                .spaces
                .get(&ws)
                .and_then(|s| s.content.active_tab())
                .is_some_and(|t| t.id == tab)
    }

    pub(crate) fn title_of(&self, ws: WorkspaceId, tab: TabId) -> String {
        self.tab(ws, tab)
            .map(|t| t.title.clone())
            .unwrap_or_default()
    }

    /// Store a raw terminal title. True when the visible title changed.
    pub(crate) fn set_title(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        raw: &str,
        spawn_dir: &str,
    ) -> bool {
        let mut clean = clean_terminal_title(raw, spawn_dir);
        if clean == "terminal" && !raw.is_empty() {
            clean = shell_name();
        }
        match self.tab_mut(ws, tab) {
            Some(t) if t.title != clean => {
                t.title = clean;
                true
            }
            _ => false,
        }
    }

    /// One output batch. True when the terminal just became "working".
    pub(crate) fn on_output(&mut self, ws: WorkspaceId, tab: TabId) -> bool {
        let title = self.title_of(ws, tab);
        self.activity
            .entry((ws, tab))
            .or_default()
            .on_output(&title)
    }

    /// Output went quiet. True when the status changed (settled, maybe flagged).
    pub(crate) fn on_quiet(&mut self, ws: WorkspaceId, tab: TabId) -> bool {
        let title = self.title_of(ws, tab);
        let was_working = self.is_working(ws, tab);
        let finished = self.activity.entry((ws, tab)).or_default().on_quiet(&title);
        let flagged = finished
            && !self.is_viewed(ws, tab)
            && self.attention.flag(ws, tab, AttentionReason::IdleSettled);
        was_working || flagged
    }

    pub(crate) fn on_bell(&mut self, ws: WorkspaceId, tab: TabId) -> bool {
        !self.is_viewed(ws, tab) && self.attention.flag(ws, tab, AttentionReason::Bell)
    }

    /// The user typed in this terminal. True when a mark or pulse went away.
    pub(crate) fn on_interaction(&mut self, ws: WorkspaceId, tab: TabId) -> bool {
        let was_working = self.activity.entry((ws, tab)).or_default().on_interaction();
        self.attention.clear_tab(ws, tab) || was_working
    }

    pub(crate) fn on_exit(&mut self, ws: WorkspaceId, tab: TabId, code: i32) {
        self.activity.entry((ws, tab)).or_default().on_exit();
        if let Some(t) = self.tab_mut(ws, tab) {
            t.exited = Some(code);
        }
    }

    /// Make `pane` the focused one. True when that changed anything.
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

    pub(crate) fn dismiss_viewed(&mut self) -> bool {
        let Some(ws) = self.active() else {
            return false;
        };
        let Some(tab) = self
            .spaces
            .get(&ws)
            .and_then(|s| s.content.active_tab())
            .map(|t| t.id)
        else {
            return false;
        };
        self.is_viewed(ws, tab) && self.attention.clear_tab(ws, tab)
    }

    pub(crate) fn is_working(&self, ws: WorkspaceId, tab: TabId) -> bool {
        self.activity
            .get(&(ws, tab))
            .is_some_and(|a| a.is_working())
    }

    pub(crate) fn tab_dot(&self, ws: WorkspaceId, tab: TabId) -> Option<WorkspaceDot> {
        workspace_dot(self.is_working(ws, tab), self.attention.reason(ws, tab))
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
                tabs.push((
                    self.is_working(ws, tab.id),
                    self.attention.reason(ws, tab.id),
                ));
            }
        });
        workspace_dot_from_tabs(tabs)
    }

    pub(crate) fn any_working(&self) -> bool {
        self.activity.values().any(|a| a.is_working())
    }
}

fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "shell".into())
}
