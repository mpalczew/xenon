//! What the phone's lists show: workspaces (open, then recent) with the same
//! status dots as the sidebar. Both frontends pass in what only they know
//! (status and terminal count of an open workspace).

use std::path::Path;

use xenon_core::{Registry, WorkspaceDot, WorkspaceId};

use crate::protocol::{Dot, WorkspaceInfo};

/// Closed workspaces offered under "Recent".
const RECENT_LIMIT: usize = 20;

/// The sidebar pip as the phone draws it.
pub fn wire_dot(dot: WorkspaceDot) -> Dot {
    match dot {
        WorkspaceDot::Working => Dot::Working,
        WorkspaceDot::Attention(_) => Dot::Attention,
    }
}

/// Open workspaces in sidebar order, then the most recently opened closed ones.
/// `live` gives an open workspace's dot and terminal count.
pub fn list_workspaces(
    registry: &Registry,
    live: impl Fn(WorkspaceId) -> (Option<WorkspaceDot>, usize),
) -> Vec<WorkspaceInfo> {
    let open = registry.workspaces.iter().map(|w| {
        let (dot, terminals) = live(w.id);
        WorkspaceInfo {
            id: w.id.to_string(),
            name: w.name.clone(),
            open: true,
            root: tilde(&w.root),
            dot: dot.map(wire_dot),
            terminals,
        }
    });
    let mut closed: Vec<_> = registry.closed_workspaces.iter().collect();
    closed.sort_by_key(|w| std::cmp::Reverse(w.last_opened.unwrap_or(0)));
    let recent = closed
        .into_iter()
        .take(RECENT_LIMIT)
        .map(|w| WorkspaceInfo {
            id: w.id.to_string(),
            name: w.name.clone(),
            open: false,
            root: tilde(&w.root),
            dot: None,
            terminals: 0,
        });
    open.chain(recent).collect()
}

/// A workspace id as the phone sends it back.
pub fn parse_workspace_id(s: &str) -> Result<WorkspaceId, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| "invalid workspace id".to_string())
}

/// `$HOME` shown as `~`.
pub fn tilde(path: &Path) -> String {
    let path = path.display().to_string();
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|rest| format!("~{rest}")))
        .unwrap_or(path)
}
