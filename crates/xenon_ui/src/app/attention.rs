//! Per-terminal-tab attention (done/bell) and working (agent-sized burst).
//! The rules live in `xenon_core::attention`; this is the GPUI-facing glue.

use super::*;
use xenon_core::{AttentionReason, TabId, WorkspaceDot, workspace_dot_from_tabs};

impl XenonApp {
    pub(super) fn flag_attention(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        reason: AttentionReason,
        cx: &mut Context<Self>,
    ) {
        if self.attention.flag(workspace, tab, reason) {
            cx.notify();
        }
    }

    pub(crate) fn clear_tab_attention(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        cx: &mut Context<Self>,
    ) {
        if self.attention.clear_tab(workspace, tab) {
            cx.notify();
        }
    }

    pub(crate) fn tab_attention(
        &self,
        workspace: WorkspaceId,
        tab: TabId,
    ) -> Option<AttentionReason> {
        self.attention.reason(workspace, tab)
    }

    /// Dismiss the mark on the tab the user is actually looking at — not every
    /// terminal in the workspace.
    pub(crate) fn dismiss_viewed_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.active else {
            return;
        };
        let Some(tab) = self
            .contents
            .get(&workspace)
            .and_then(|c| c.active_tab())
            .and_then(|tab| match tab {
                LiveTab::Terminal { id, .. } => Some(*id),
                _ => None,
            })
        else {
            return;
        };
        self.clear_tab_attention(workspace, tab, cx);
    }

    pub(crate) fn workspace_status(&self, id: WorkspaceId) -> Option<WorkspaceDot> {
        #[cfg(feature = "visual-tests")]
        if let Some(dot) = self.services.visual_status.get(&id) {
            return Some(*dot);
        }
        let root = self.contents.get(&id).and_then(|c| c.root.as_ref())?;
        let mut tabs = Vec::new();
        root.for_each_terminal(&mut |tab, view| {
            tabs.push((self.terminal_working(view), self.attention.reason(id, tab)));
        });
        workspace_dot_from_tabs(tabs)
    }

    /// Cached chrome summary for a terminal view; drawing reads this, never the view.
    pub(crate) fn terminal_chrome(&self, view: &Entity<TerminalView>) -> Option<&TerminalChrome> {
        self.terminal_chrome.get(&view.entity_id())
    }

    pub(crate) fn terminal_working(&self, view: &Entity<TerminalView>) -> bool {
        self.terminal_chrome(view)
            .is_some_and(|chrome| chrome.working)
    }

    /// Redraw only when the change is visible: status flips show in the sidebar
    /// everywhere, titles and phone owners only on the active workspace's chips.
    pub(super) fn on_terminal_chrome(
        &mut self,
        entity: gpui::EntityId,
        owner: Option<(WorkspaceId, TabId)>,
        chrome: TerminalChrome,
        cx: &mut Context<Self>,
    ) {
        let old = self.terminal_chrome.insert(entity, chrome.clone());
        let status_changed = old
            .as_ref()
            .is_none_or(|old| old.working != chrome.working || old.exited != chrome.exited);
        let on_screen = owner.is_some_and(|(workspace, _)| self.active == Some(workspace));
        if status_changed || on_screen {
            cx.notify();
        }
    }

    /// Always notify: a sibling can keep the workspace working while this tab
    /// flips between pulse, done, and quiet.
    pub(super) fn refresh_terminal_status(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
}
