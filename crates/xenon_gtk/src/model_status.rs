//! Terminal status on the model: working, attention, and exit, built from the
//! shared rules in `xenon_core`. The views report raw signals (output batch,
//! quiet, bell, key, exit); this decides what they mean.

use xenon_core::{
    AttentionReason, TabId, WorkspaceDot, WorkspaceId, clean_terminal_title, workspace_dot,
    workspace_dot_from_tabs,
};

use crate::model::{Model, Tab, TabKind};

impl Model {
    /// The tab is on screen in the focused pane of the open workspace while
    /// the window has focus.
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
            // The prompt titled itself with the folder it started in: name the
            // shell, which `terminal_tab_label` numbers like the GPUI app.
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

    /// Dismiss the mark on the tab the user is looking at.
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

    /// One terminal's pip: the same rule the tab chip and the sidebar share.
    pub(crate) fn tab_dot(&self, ws: WorkspaceId, tab: TabId) -> Option<WorkspaceDot> {
        workspace_dot(self.is_working(ws, tab), self.attention.reason(ws, tab))
    }

    /// The workspace row's pip: aggregate over its terminals.
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

    /// True when any terminal anywhere is pulsing (the ticker needs to run).
    pub(crate) fn any_working(&self) -> bool {
        self.activity.values().any(|a| a.is_working())
    }
}

/// Name of the user's shell (`zsh`), for tabs that show a plain prompt.
fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "shell".into())
}
