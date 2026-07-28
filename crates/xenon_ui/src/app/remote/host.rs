//! Host-side fulfillment for mobile remote list/frame/inject.

use super::*;
use xenon_core::{PaneNode, TabId, TabState, WorkspaceId};
use xenon_remote::{TerminalInfo, ViewportSnapshot, WorkspaceInfo, next_global_seq};

impl XenonApp {
    /// Open + closed (recent) workspaces for the phone picker.
    pub(super) fn list_remote_workspaces(&self) -> Vec<WorkspaceInfo> {
        let mut list: Vec<WorkspaceInfo> = self
            .registry
            .workspaces
            .iter()
            .map(|w| WorkspaceInfo {
                id: w.id.to_string(),
                name: w.name.clone(),
                open: self.contents.contains_key(&w.id),
                root: w.root.display().to_string(),
            })
            .collect();
        // Closed archive as recents (sorted by last_opened desc).
        let mut closed: Vec<_> = self.registry.closed_workspaces.iter().collect();
        closed.sort_by_key(|b| std::cmp::Reverse(b.last_opened.unwrap_or(0)));
        for w in closed.into_iter().take(20) {
            list.push(WorkspaceInfo {
                id: w.id.to_string(),
                name: w.name.clone(),
                open: false,
                root: w.root.display().to_string(),
            });
        }
        list
    }

    /// Ensure workspace is live (activate open / reopen closed), then list terminals.
    pub(super) fn list_remote_terminals(
        &mut self,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<Vec<TerminalInfo>, String> {
        let wid = parse_workspace_id(workspace_id)?;
        self.ensure_workspace_live(wid, cx)?;
        // Closed → reopened PTYs start at zed's tiny default until first paint.
        // Force a real grid so remote frames match an opened workspace.
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

        let mut out = Vec::new();
        collect_terminals(root, &mut |id, view| {
            let title = view.read(cx).title(cx);
            out.push(TerminalInfo {
                tab_id: id.0,
                title: Some(title),
                cwd: String::new(),
                active: active_tab == Some(id),
            });
        });
        if let Some(session) = self.sessions.get(&wid)
            && let Some(layout_root) = session.content.root.as_ref()
        {
            for t in &mut out {
                if let Some(cwd) = find_terminal_cwd(layout_root, TabId(t.tab_id)) {
                    t.cwd = cwd;
                }
            }
        }
        Ok(out)
    }

    pub(super) fn ensure_workspace_live(
        &mut self,
        wid: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.contents.contains_key(&wid) {
            return Ok(());
        }
        if self.registry.workspace(wid).is_some() {
            self.activate_workspace(wid, cx);
            if self.contents.contains_key(&wid) {
                return Ok(());
            }
            return Err("could not activate workspace".into());
        }
        if self.registry.closed_workspaces.iter().any(|w| w.id == wid) {
            self.reopen_workspace(wid, cx);
            if self.contents.contains_key(&wid) {
                return Ok(());
            }
            return Err("could not reopen workspace".into());
        }
        Err("unknown workspace".into())
    }

    /// Best grid size from any already-laid-out terminal, else a solid remote default.
    pub(super) fn reference_grid_size(&self, cx: &App) -> (u16, u16) {
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
            // Comfortable default when nothing has painted yet (closed workspace).
            (120, 40)
        }
    }

    /// Resize undersized PTYs in `wid` (reopen-before-paint path).
    pub(super) fn ensure_remote_terminal_sizes(
        &mut self,
        wid: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        let (cols, rows) = self.reference_grid_size(cx);
        let views: Vec<Entity<TerminalView>> = {
            let Some(content) = self.contents.get(&wid) else {
                return;
            };
            let Some(root) = content.root.as_ref() else {
                return;
            };
            let mut out = Vec::new();
            collect_terminals(root, &mut |_, view| {
                if view.read(cx).needs_layout_size(cx) {
                    out.push(view.clone());
                }
            });
            out
        };
        for view in views {
            view.update(cx, |term, cx| term.ensure_grid_size(cols, rows, cx));
        }
    }

    pub(super) fn capture_remote_frame(
        &mut self,
        workspace_id: &str,
        tab_id: u64,
        cx: &mut Context<Self>,
    ) -> Result<ViewportSnapshot, String> {
        let wid = parse_workspace_id(workspace_id)?;
        // Don't activate here (paint path every 200ms) — select/list already did.
        // Still heal size if this tab was never laid out on the desktop.
        let needs = self
            .find_terminal_view(wid, TabId(tab_id))
            .is_some_and(|v| v.read(cx).needs_layout_size(cx));
        if needs {
            self.ensure_remote_terminal_sizes(wid, cx);
        }
        let view = self
            .find_terminal_view(wid, TabId(tab_id))
            .ok_or_else(|| "terminal not found".to_string())?;
        if view.read(cx).is_exited() {
            return Err("terminal closed".to_string());
        }
        let (cols, rows, cells) = view
            .read(cx)
            .viewport_cells(cx)
            .ok_or_else(|| "terminal not ready".to_string())?;
        // Shared pure assembler: viewport only (scrollback rows discarded).
        let lines = xenon_remote::viewport_lines(cells, cols, rows);
        let seq = {
            let mut map = self
                .services
                .remote_frame_seq
                .lock()
                .expect("remote seq lock");
            let e = map.entry((wid, tab_id)).or_insert(0);
            *e = e.wrapping_add(1).max(1);
            let _ = next_global_seq();
            *e
        };
        Ok(ViewportSnapshot {
            tab_id,
            seq,
            cols,
            rows,
            lines,
        })
    }

    pub(super) fn inject_remote(
        &mut self,
        workspace_id: &str,
        tab_id: u64,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let wid = parse_workspace_id(workspace_id)?;
        self.ensure_workspace_live(wid, cx)?;
        let view = self
            .find_terminal_view(wid, TabId(tab_id))
            .ok_or_else(|| "terminal not found".to_string())?
            .clone();
        // inject_text only writes PTY bytes — never resizes.
        view.update(cx, |term, cx| term.inject_text(text, cx));
        Ok(())
    }

    pub(super) fn find_terminal_view(
        &self,
        workspace: WorkspaceId,
        tab: TabId,
    ) -> Option<&Entity<TerminalView>> {
        let content = self.contents.get(&workspace)?;
        let root = content.root.as_ref()?;
        let (pane, idx) = root.find_tab(tab)?;
        let leaf = root.find_leaf(pane)?;
        leaf.tabs.get(idx)?.as_terminal()
    }
}

pub(super) fn parse_workspace_id(s: &str) -> Result<WorkspaceId, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| "invalid workspace id".to_string())
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

pub(super) fn find_terminal_cwd(node: &PaneNode, tab: TabId) -> Option<String> {
    match node {
        PaneNode::Leaf(leaf) => {
            for t in &leaf.tabs {
                if let TabState::Terminal { id, cwd } = t
                    && *id == tab
                {
                    return Some(cwd.display().to_string());
                }
            }
            None
        }
        PaneNode::Split { first, second, .. } => {
            find_terminal_cwd(first, tab).or_else(|| find_terminal_cwd(second, tab))
        }
    }
}
