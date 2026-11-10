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
