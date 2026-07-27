//! XenonApp wiring for unified terminal/editor location history.

use super::nav_history::NavEntry;
use super::*;
use xenon_core::{TabId, WorkspaceId};

impl XenonApp {
    pub(crate) fn nav_visit(&mut self, _tab: TabId, cx: &App) {
        if self.nav_suppress {
            return;
        }
        let Some(ws) = self.active else {
            return;
        };
        let Some(entry) = self.active_nav_entry(ws, cx) else {
            return;
        };
        self.nav_history.entry(ws).or_default().visit(entry);
    }

    pub(crate) fn nav_seed_active(&mut self, ws: WorkspaceId, cx: &App) {
        let Some(entry) = self.active_nav_entry(ws, cx) else {
            return;
        };
        self.nav_history.entry(ws).or_default().seed_if_empty(entry);
    }

    pub(crate) fn nav_sync_active(&mut self, ws: WorkspaceId, cx: &App) {
        if self.nav_suppress {
            return;
        }
        let Some(entry) = self.active_nav_entry(ws, cx) else {
            return;
        };
        let history = self.nav_history.entry(ws).or_default();
        if history.is_empty() {
            history.seed_if_empty(entry);
        } else {
            history.refresh_current(entry);
        }
    }

    pub(crate) fn nav_record_definition(
        &mut self,
        ws: WorkspaceId,
        origin: NavEntry,
        destination: NavEntry,
    ) {
        let history = self.nav_history.entry(ws).or_default();
        history.refresh_current(origin);
        history.visit(destination);
    }

    pub(crate) fn nav_prune_tab(&mut self, ws: WorkspaceId, tab: TabId) {
        if let Some(history) = self.nav_history.get_mut(&ws) {
            history.prune_terminal(tab);
        }
    }

    pub(super) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_history(-1, window, cx);
    }

    pub(super) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_history(1, window, cx);
    }

    fn go_history(&mut self, direction: i8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ws) = self.active else {
            return;
        };
        self.nav_sync_active(ws, cx);
        self.prune_dead_nav(ws);
        let target = {
            let history = self.nav_history.entry(ws).or_default();
            if direction < 0 {
                history.back()
            } else {
                history.forward()
            }
        };
        let Some(target) = target else {
            return;
        };
        self.nav_suppress = true;
        let restored = self.restore_nav_entry(ws, &target, window, cx);
        self.nav_suppress = false;
        if !restored {
            self.prune_dead_nav(ws);
        }
    }

    fn active_nav_entry(&self, ws: WorkspaceId, cx: &App) -> Option<NavEntry> {
        match self.contents.get(&ws)?.active_tab()? {
            LiveTab::Terminal { id, .. } => Some(NavEntry::Terminal { tab: *id }),
            LiveTab::Editor { path, view, .. } => {
                let (row, col) = view.read(cx).cursor_position()?;
                Some(NavEntry::Editor {
                    path: path.clone(),
                    row,
                    col,
                })
            }
        }
    }

    fn restore_nav_entry(
        &mut self,
        ws: WorkspaceId,
        entry: &NavEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match entry {
            NavEntry::Terminal { tab } => {
                let location = self
                    .contents
                    .get(&ws)
                    .and_then(|content| content.root.as_ref())
                    .and_then(|root| root.find_tab(*tab));
                let Some((pane, index)) = location else {
                    return false;
                };
                self.activate_tab_in_pane(pane, index, window, cx);
                true
            }
            NavEntry::Editor { path, row, col } => self
                .open_editor_at(path.clone(), true, Some((*row, *col)), cx)
                .is_ok(),
        }
    }

    fn prune_dead_nav(&mut self, ws: WorkspaceId) {
        let live_terminals = self
            .contents
            .get(&ws)
            .and_then(|content| content.root.as_ref())
            .map(|root| {
                let mut ids = std::collections::HashSet::new();
                collect_terminal_ids(root, &mut ids);
                ids
            })
            .unwrap_or_default();
        if let Some(history) = self.nav_history.get_mut(&ws) {
            history.retain(|entry| match entry {
                NavEntry::Editor { path, .. } => path.is_file(),
                NavEntry::Terminal { tab } => live_terminals.contains(tab),
            });
        }
    }
}

fn collect_terminal_ids(node: &LiveNode, out: &mut std::collections::HashSet<TabId>) {
    match node {
        LiveNode::Leaf(leaf) => {
            for tab in &leaf.tabs {
                if matches!(tab, LiveTab::Terminal { .. }) {
                    out.insert(tab.id());
                }
            }
        }
        LiveNode::Split { first, second, .. } => {
            collect_terminal_ids(first, out);
            collect_terminal_ids(second, out);
        }
    }
}
