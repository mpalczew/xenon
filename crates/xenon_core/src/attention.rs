//! Terminal tab status: attention (done/bell) and working, and the workspace
//! row derived from its tabs so sidebar and chips cannot disagree.

/// Why a tab attention badge is lit. Last write wins on that tab; for debug tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionReason {
    Bell,
    IdleSettled,
}

impl AttentionReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::IdleSettled => "Idle after busy output",
        }
    }
}

/// Terminal tab status. Working outranks a settled attention mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceDot {
    Working,
    Attention(&'static str),
}

impl WorkspaceDot {
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Attention(label) => label,
        }
    }
}

/// One terminal tab's visible pip. Shared by the tab chip and the workspace aggregate.
pub fn workspace_dot(working: bool, attention: Option<AttentionReason>) -> Option<WorkspaceDot> {
    if working {
        Some(WorkspaceDot::Working)
    } else {
        attention.map(|reason| WorkspaceDot::Attention(reason.label()))
    }
}

/// Workspace row: any working → working; else any attention → attention; else none.
pub fn workspace_dot_from_tabs(
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

#[cfg(test)]
mod tests {
    use super::{AttentionReason, WorkspaceDot, workspace_dot, workspace_dot_from_tabs};
    use crate::attention_map::AttentionMap;
    use crate::{TabId, WorkspaceId};

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
}
