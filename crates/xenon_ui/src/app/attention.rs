//! Per-terminal-tab attention (done/bell) and working (agent-sized burst).
//! The workspace row is derived from those tabs so sidebar and chips cannot disagree.

use std::collections::HashMap;

use super::*;
use xenon_core::TabId;

/// Why a tab attention badge is lit. Last write wins on that tab; for debug tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttentionReason {
    Bell,
    IdleSettled,
}

impl AttentionReason {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::IdleSettled => "Idle after busy output",
        }
    }
}

/// Sidebar / tab status pip. Working outranks a settled attention mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceDot {
    Working,
    Attention(&'static str),
}

impl WorkspaceDot {
    pub(crate) fn tooltip(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Attention(label) => label,
        }
    }

    pub(crate) fn pip(self, cx: &App) -> impl IntoElement {
        match self {
            Self::Working => crate::chrome::status_pip(crate::chrome::working_color(cx), true),
            Self::Attention(_) => {
                crate::chrome::status_pip(crate::chrome::attention_color(cx), false)
            }
        }
    }
}

/// One terminal tab's visible pip. Shared by the tab chip and the workspace aggregate.
pub(crate) fn workspace_dot(
    working: bool,
    attention: Option<AttentionReason>,
) -> Option<WorkspaceDot> {
    if working {
        Some(WorkspaceDot::Working)
    } else {
        attention.map(|reason| WorkspaceDot::Attention(reason.label()))
    }
}

/// Workspace row: any working → working; else any attention → attention; else none.
pub(crate) fn workspace_dot_from_tabs(
    tabs: impl IntoIterator<Item = (bool, Option<AttentionReason>)>,
) -> Option<WorkspaceDot> {
    let mut any_working = false;
    let mut attention = None;
    for (working, reason) in tabs {
        any_working |= working;
        if reason.is_some() {
            attention = reason;
        }
    }
    workspace_dot(any_working, attention)
}

/// Attention marks keyed by terminal tab. Working is live on the terminal view.
#[derive(Clone, Debug, Default)]
pub(crate) struct AttentionMap {
    marks: HashMap<(WorkspaceId, TabId), AttentionReason>,
}

impl AttentionMap {
    pub(crate) fn flag(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        reason: AttentionReason,
    ) -> bool {
        !matches!(self.marks.insert((workspace, tab), reason), Some(prev) if prev == reason)
    }

    pub(crate) fn clear_tab(&mut self, workspace: WorkspaceId, tab: TabId) -> bool {
        self.marks.remove(&(workspace, tab)).is_some()
    }

    pub(crate) fn clear_workspace(&mut self, workspace: WorkspaceId) -> bool {
        let before = self.marks.len();
        self.marks.retain(|(ws, _), _| *ws != workspace);
        self.marks.len() != before
    }

    pub(crate) fn reason(&self, workspace: WorkspaceId, tab: TabId) -> Option<AttentionReason> {
        self.marks.get(&(workspace, tab)).copied()
    }
}

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

    pub(crate) fn workspace_status(&self, id: WorkspaceId, cx: &App) -> Option<WorkspaceDot> {
        let root = self.contents.get(&id).and_then(|c| c.root.as_ref())?;
        let mut tabs = Vec::new();
        root.for_each_terminal(&mut |tab, view| {
            tabs.push((view.read(cx).is_working(), self.attention.reason(id, tab)));
        });
        workspace_dot_from_tabs(tabs)
    }

    /// Always notify: a sibling can keep the workspace working while this tab
    /// flips between pulse, done, and quiet.
    pub(super) fn refresh_terminal_status(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttentionMap, AttentionReason, WorkspaceDot, workspace_dot, workspace_dot_from_tabs,
    };
    use xenon_core::{TabId, WorkspaceId};

    fn tab_states(
        map: &AttentionMap,
        workspace: WorkspaceId,
        tabs: &[(TabId, bool)],
    ) -> Vec<(bool, Option<AttentionReason>)> {
        tabs.iter()
            .map(|(tab, working)| (*working, map.reason(workspace, *tab)))
            .collect()
    }

    #[test]
    fn working_dot_wins_over_attention() {
        assert_eq!(
            workspace_dot(true, Some(AttentionReason::IdleSettled)),
            Some(WorkspaceDot::Working)
        );
        assert_eq!(
            workspace_dot(false, Some(AttentionReason::Bell)),
            Some(WorkspaceDot::Attention("Bell"))
        );
        assert_eq!(
            workspace_dot(false, Some(AttentionReason::IdleSettled)),
            Some(WorkspaceDot::Attention("Idle after busy output"))
        );
        assert_eq!(workspace_dot(false, None), None);
    }

    #[test]
    fn finished_or_bell_is_attention_when_not_working() {
        let mut map = AttentionMap::default();
        let ws = WorkspaceId::new();
        let tab = TabId(1);
        map.flag(ws, tab, AttentionReason::IdleSettled);
        assert_eq!(
            workspace_dot(false, map.reason(ws, tab)),
            Some(WorkspaceDot::Attention("Idle after busy output"))
        );
        map.flag(ws, tab, AttentionReason::Bell);
        assert_eq!(
            workspace_dot(false, map.reason(ws, tab)),
            Some(WorkspaceDot::Attention("Bell"))
        );
        assert_eq!(
            workspace_dot(true, map.reason(ws, tab)),
            Some(WorkspaceDot::Working)
        );
    }

    #[test]
    fn terminals_stay_independent_and_workspace_aggregates() {
        let mut map = AttentionMap::default();
        let ws = WorkspaceId::new();
        let a = TabId(1);
        let b = TabId(2);

        map.flag(ws, a, AttentionReason::IdleSettled);
        let a_done_b_working = tab_states(&map, ws, &[(a, false), (b, true)]);
        assert_eq!(
            workspace_dot(false, map.reason(ws, a)),
            Some(WorkspaceDot::Attention("Idle after busy output"))
        );
        assert_eq!(
            workspace_dot(true, map.reason(ws, b)),
            Some(WorkspaceDot::Working)
        );
        assert_eq!(
            workspace_dot_from_tabs(a_done_b_working),
            Some(WorkspaceDot::Working)
        );

        map.clear_tab(ws, a);
        assert_eq!(map.reason(ws, a), None);
        assert_eq!(map.reason(ws, b), None);
        map.flag(ws, b, AttentionReason::Bell);
        assert_eq!(map.reason(ws, a), None);
        assert_eq!(map.reason(ws, b), Some(AttentionReason::Bell));
        assert_eq!(
            workspace_dot_from_tabs(tab_states(&map, ws, &[(a, false), (b, false)])),
            Some(WorkspaceDot::Attention("Bell"))
        );
        assert_eq!(
            workspace_dot_from_tabs(tab_states(&map, ws, &[(a, false), (b, false)])),
            workspace_dot(false, map.reason(ws, b))
        );
    }

    #[test]
    fn dismiss_one_tab_does_not_clear_sibling() {
        let mut map = AttentionMap::default();
        let ws = WorkspaceId::new();
        let a = TabId(1);
        let b = TabId(2);
        map.flag(ws, a, AttentionReason::IdleSettled);
        map.flag(ws, b, AttentionReason::Bell);
        assert!(map.clear_tab(ws, a));
        assert_eq!(map.reason(ws, a), None);
        assert_eq!(map.reason(ws, b), Some(AttentionReason::Bell));
        assert_eq!(
            workspace_dot_from_tabs(tab_states(&map, ws, &[(a, false), (b, false)])),
            Some(WorkspaceDot::Attention("Bell"))
        );
    }

    #[test]
    fn clear_workspace_drops_only_that_workspace() {
        let mut map = AttentionMap::default();
        let ws_a = WorkspaceId::new();
        let ws_b = WorkspaceId::new();
        let tab = TabId(1);
        map.flag(ws_a, tab, AttentionReason::Bell);
        map.flag(ws_b, tab, AttentionReason::IdleSettled);
        assert!(map.clear_workspace(ws_a));
        assert_eq!(map.reason(ws_a, tab), None);
        assert_eq!(map.reason(ws_b, tab), Some(AttentionReason::IdleSettled));
    }
}
