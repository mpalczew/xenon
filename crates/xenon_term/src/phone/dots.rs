//! Attention and working dots, pushed to every phone when they change.

use std::collections::{BTreeMap, HashMap};

use xenon_core::{AttentionReason, WorkspaceId, workspace_dot, workspace_dot_from_tabs};
use xenon_remote::{Dot, ServerMsg, wire_dot};

use super::{Phone, locked};

type Tab = (WorkspaceId, u64, bool, Option<AttentionReason>);

impl Phone {
    /// Every terminal: workspace, phone tab number, working, attention.
    fn tabs(&self) -> Vec<Tab> {
        self.terms
            .all()
            .iter()
            .map(|t| {
                let s = t.info().status;
                (t.workspace, t.serial, s.working, s.attention)
            })
            .collect()
    }

    /// The dot of a workspace and how many terminals it has.
    pub(crate) fn workspace_summary(
        &self,
        ws: WorkspaceId,
    ) -> (Option<xenon_core::WorkspaceDot>, usize) {
        let tabs: Vec<_> = self.tabs().into_iter().filter(|t| t.0 == ws).collect();
        let dot = workspace_dot_from_tabs(tabs.iter().map(|t| (t.2, t.3)));
        (dot, tabs.len())
    }

    pub(crate) fn push_dots(&self) {
        if locked(&self.conns).is_empty() {
            return;
        }
        let tabs = self.tabs();
        let mut by_ws: HashMap<WorkspaceId, Vec<(bool, Option<AttentionReason>)>> = HashMap::new();
        for t in &tabs {
            by_ws.entry(t.0).or_default().push((t.2, t.3));
        }
        let workspaces: BTreeMap<String, Dot> = by_ws
            .into_iter()
            .filter_map(|(ws, list)| {
                Some((ws.to_string(), wire_dot(workspace_dot_from_tabs(list)?)))
            })
            .collect();
        for conn in locked(&self.conns).values_mut() {
            let attached = conn.attach.as_ref().map(|a| a.term.workspace);
            let tabs = tabs
                .iter()
                .filter(|t| Some(t.0) == attached)
                .filter_map(|t| Some((t.1, wire_dot(workspace_dot(t.2, t.3)?))))
                .collect();
            let msg = ServerMsg::Dots {
                workspaces: workspaces.clone(),
                workspace: attached.map(|ws| ws.to_string()),
                tabs,
            };
            if conn.last_dots.as_ref() != Some(&msg) && conn.out.try_send(msg.clone()) {
                conn.last_dots = Some(msg);
            }
        }
    }
}
