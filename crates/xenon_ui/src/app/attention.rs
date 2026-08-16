//! Workspace attention (done/bell) and working (agent-sized burst) status.

use super::*;

/// Why a workspace attention badge is lit. Last write wins; for debug tooltips.
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

impl XenonApp {
    pub(super) fn flag_attention(
        &mut self,
        id: WorkspaceId,
        reason: AttentionReason,
        cx: &mut Context<Self>,
    ) {
        match self.attention.insert(id, reason) {
            Some(prev) if prev == reason => {}
            _ => cx.notify(),
        }
    }

    pub(crate) fn clear_attention(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.attention.remove(&id).is_some() {
            cx.notify();
        }
    }

    pub(crate) fn attention_reason(&self, id: WorkspaceId) -> Option<AttentionReason> {
        self.attention.get(&id).copied()
    }

    pub(crate) fn is_working(&self, id: WorkspaceId) -> bool {
        self.working.contains(&id)
    }

    pub(super) fn sync_working(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        let any = self
            .contents
            .get(&id)
            .and_then(|content| content.root.as_ref())
            .is_some_and(|root| {
                let mut any = false;
                root.for_each_terminal(&mut |view| {
                    if !any && view.read(cx).is_working() {
                        any = true;
                    }
                });
                any
            });
        let changed = if any {
            self.working.insert(id)
        } else {
            self.working.remove(&id)
        };
        if changed {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AttentionReason, WorkspaceDot, workspace_dot};

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
        assert_eq!(workspace_dot(false, None), None);
    }
}
