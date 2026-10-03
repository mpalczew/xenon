//! Phone lists: workspaces (open + recent) and a workspace's terminals, with
//! the same status dots as the sidebar. Opening a closed workspace from the
//! phone never changes which workspace the Mac shows.

use super::*;
use xenon_core::{PaneNode, TabState};
use xenon_remote::{Dot, TerminalInfo, WorkspaceInfo};

/// Closed workspaces offered under "Recent".
const RECENT_LIMIT: usize = 20;

pub(super) fn wire_dot(dot: WorkspaceDot) -> Dot {
    match dot {
        WorkspaceDot::Working => Dot::Working,
        WorkspaceDot::Attention(_) => Dot::Attention,
    }
}

impl XenonApp {
    pub(super) fn list_remote_workspaces(&self, cx: &App) -> Vec<WorkspaceInfo> {
        let open = self.registry.workspaces.iter().map(|w| WorkspaceInfo {
            id: w.id.to_string(),
            name: w.name.clone(),
            open: true,
            root: tilde(&w.root),
            dot: self.workspace_status(w.id, cx).map(wire_dot),
            terminals: self.terminal_count(w.id),
        });
        let mut closed: Vec<_> = self.registry.closed_workspaces.iter().collect();
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

    /// Make the workspace live if needed, then list its terminal tabs.
    pub(super) fn list_remote_terminals(
        &mut self,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<Vec<TerminalInfo>, String> {
        let wid = parse_workspace_id(workspace_id)?;
        self.ensure_workspace_live(wid, cx)?;
        self.ensure_remote_terminal_sizes(wid, cx);
        let content = self
            .contents
            .get(&wid)
            .ok_or_else(|| "workspace failed to open".to_string())?;
        let Some(root) = content.root.as_ref() else {
            return Ok(Vec::new());
        };
        let active_tab = content
            .focused
            .and_then(|pane| root.find_leaf(pane))
            .and_then(|leaf| leaf.active_tab())
            .map(|t| t.id());
        let dots = self.remote_terminal_dots(wid, cx);
        let layout = self
            .sessions
            .get(&wid)
            .and_then(|s| s.content.root.as_ref());
        let mut out = Vec::new();
        collect_terminals(root, &mut |id, view| {
            out.push(TerminalInfo {
                tab_id: id.0,
                title: Some(view.read(cx).title(cx)),
                cwd: layout
                    .and_then(|l| find_terminal_cwd(l, id))
                    .unwrap_or_default(),
                active: active_tab == Some(id),
                dot: dots.get(&id.0).copied(),
            });
        });
        Ok(out)
    }

    /// New shell in this workspace. The Mac keeps its current workspace and tab.
    pub(super) fn open_remote_terminal(
        &mut self,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<TerminalInfo, String> {
        let wid = parse_workspace_id(workspace_id)?;
        self.ensure_workspace_live(wid, cx)?;
        let root = self
            .workspace_root(wid)
            .ok_or_else(|| "unknown workspace".to_string())?;
        let view = self.spawn_terminal(root.clone(), wid, cx);
        let tab_id = self.insert_terminal_tab(wid, None, view, false);
        self.save_layout(wid);
        self.ensure_remote_terminal_sizes(wid, cx);
        let active = self
            .contents
            .get(&wid)
            .and_then(|c| c.active_tab())
            .is_some_and(|t| t.id() == tab_id);
        let title = self
            .find_terminal_view(wid, tab_id)
            .map(|view| view.read(cx).title(cx));
        cx.notify();
        Ok(TerminalInfo {
            tab_id: tab_id.0,
            title,
            cwd: tilde(&root),
            active,
            dot: None,
        })
    }

    /// Per-terminal dots for one workspace (working outranks attention).
    pub(super) fn remote_terminal_dots(&self, wid: WorkspaceId, cx: &App) -> BTreeMap<u64, Dot> {
        let mut dots = BTreeMap::new();
        if let Some(root) = self.contents.get(&wid).and_then(|c| c.root.as_ref()) {
            collect_terminals(root, &mut |id, view| {
                let dot = workspace_dot(view.read(cx).is_working(), self.tab_attention(wid, id));
                if let Some(dot) = dot {
                    dots.insert(id.0, wire_dot(dot));
                }
            });
        }
        dots
    }

    fn terminal_count(&self, wid: WorkspaceId) -> usize {
        let mut n = 0;
        if let Some(root) = self.contents.get(&wid).and_then(|c| c.root.as_ref()) {
            collect_terminals(root, &mut |_, _| n += 1);
        }
        n
    }

    /// Load a workspace's terminals without touching the Mac's screen: no
    /// activation, so the Mac's workspace, pickers, and focus stay put.
    pub(super) fn ensure_workspace_live(
        &mut self,
        wid: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.contents.contains_key(&wid) {
            return Ok(());
        }
        let closed = self.registry.closed_workspaces.iter().any(|w| w.id == wid);
        if closed && !self.restore_closed_workspace(wid, cx) {
            return Err("could not reopen workspace".into());
        }
        let root = self
            .workspace_root(wid)
            .ok_or_else(|| "unknown workspace".to_string())?;
        self.ensure_live_content(wid, &root, cx);
        Ok(())
    }

    /// Best grid size from any laid-out terminal, else a comfortable default.
    fn reference_grid_size(&self, cx: &App) -> (u16, u16) {
        let mut best = (0u16, 0u16);
        for content in self.contents.values() {
            let Some(root) = content.root.as_ref() else {
                continue;
            };
            collect_terminals(root, &mut |_, view| {
                if let Some((c, r)) = view.read(cx).grid_size(cx)
                    && c >= 40
                    && r >= 12
                    && c.saturating_mul(r) > best.0.saturating_mul(best.1)
                {
                    best = (c, r);
                }
            });
        }
        if best.0 >= 40 && best.1 >= 12 {
            best
        } else {
            (120, 40)
        }
    }

    /// Terminals that never painted (reopened for the phone) start tiny: size them.
    pub(super) fn ensure_remote_terminal_sizes(
        &mut self,
        wid: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        let (cols, rows) = self.reference_grid_size(cx);
        let mut views = Vec::new();
        if let Some(root) = self.contents.get(&wid).and_then(|c| c.root.as_ref()) {
            collect_terminals(root, &mut |_, view| {
                if view.read(cx).needs_layout_size(cx) {
                    views.push(view.clone());
                }
            });
        }
        for view in views {
            view.update(cx, |term, cx| term.ensure_grid_size(cols, rows, cx));
        }
    }

    pub(super) fn find_terminal_view(
        &self,
        workspace: WorkspaceId,
        tab: TabId,
    ) -> Option<&Entity<TerminalView>> {
        let root = self.contents.get(&workspace)?.root.as_ref()?;
        let (pane, idx) = root.find_tab(tab)?;
        root.find_leaf(pane)?.tabs.get(idx)?.as_terminal()
    }
}

pub(super) fn parse_workspace_id(s: &str) -> Result<WorkspaceId, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| "invalid workspace id".to_string())
}

fn tilde(path: &Path) -> String {
    let path = path.display().to_string();
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|rest| format!("~{rest}")))
        .unwrap_or(path)
}

pub(super) fn collect_terminals(node: &LiveNode, f: &mut dyn FnMut(TabId, &Entity<TerminalView>)) {
    match node {
        LiveNode::Leaf(leaf) => {
            for t in &leaf.tabs {
                if let LiveTab::Terminal { id, view } = t {
                    f(*id, view);
                }
            }
        }
        LiveNode::Split { first, second, .. } => {
            collect_terminals(first, f);
            collect_terminals(second, f);
        }
    }
}

fn find_terminal_cwd(node: &PaneNode, tab: TabId) -> Option<String> {
    match node {
        PaneNode::Leaf(leaf) => leaf.tabs.iter().find_map(|t| match t {
            TabState::Terminal { id, cwd } if *id == tab => Some(cwd.display().to_string()),
            _ => None,
        }),
        PaneNode::Split { first, second, .. } => {
            find_terminal_cwd(first, tab).or_else(|| find_terminal_cwd(second, tab))
        }
    }
}
