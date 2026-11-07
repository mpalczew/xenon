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
