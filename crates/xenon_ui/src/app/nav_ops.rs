//! XenonApp wiring for tab surface history (⌘[ / ⌘]).

use super::*;
use xenon_core::{TabId, WorkspaceId};

impl XenonApp {
    /// Record the active surface after a user-driven tab change.
    pub(crate) fn nav_visit(&mut self, tab: TabId) {
        if self.nav_suppress {
            return;
        }
        let Some(ws) = self.active else {
            return;
        };
        self.nav_history.entry(ws).or_default().visit(tab);
    }

    /// Ensure the workspace history starts with its current active tab.
    pub(crate) fn nav_seed_active(&mut self, ws: WorkspaceId) {
        let tab = self
            .contents
            .get(&ws)
            .and_then(|c| c.active_tab())
            .map(|t| t.id());
        let Some(tab) = tab else {
            return;
        };
        self.nav_history.entry(ws).or_default().seed_if_empty(tab);
    }

    /// Before leaving a surface without `activate_tab_in_pane` (e.g. open_editor),
    /// put the current active tab on the stack tip.
    pub(crate) fn nav_sync_active(&mut self, ws: WorkspaceId) {
        if self.nav_suppress {
            return;
        }
        let Some(tab) = self
            .contents
            .get(&ws)
            .and_then(|c| c.active_tab())
            .map(|t| t.id())
        else {
            return;
        };
        let h = self.nav_history.entry(ws).or_default();
        if h.is_empty() {
            h.seed_if_empty(tab);
        } else if h.current() != Some(tab) {
            h.visit(tab);
        }
    }

    pub(crate) fn nav_prune_tab(&mut self, ws: WorkspaceId, tab: TabId) {
        if let Some(h) = self.nav_history.get_mut(&ws) {
            h.prune(tab);
        }
    }

    pub(super) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_history(-1, window, cx);
    }

    pub(super) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_history(1, window, cx);
    }

    fn go_history(&mut self, dir: i8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ws) = self.active else {
            return;
        };
        self.prune_dead_nav(ws);
        loop {
            let target = {
                let h = self.nav_history.entry(ws).or_default();
                if dir < 0 { h.back() } else { h.forward() }
            };
            let Some(tab) = target else {
                return;
            };
            if self.try_activate_nav_tab(ws, tab, window, cx) {
                return;
            }
            // Tab vanished between prune and step; drop it. `prune` repairs
            // index onto a kept entry - try that before stepping again.
            self.nav_prune_tab(ws, tab);
            if let Some(cur) = self.nav_history.get(&ws).and_then(|h| h.current())
                && self.try_activate_nav_tab(ws, cur, window, cx)
            {
                return;
            }
        }
    }

    fn prune_dead_nav(&mut self, ws: WorkspaceId) {
        let live: std::collections::HashSet<TabId> = self
            .contents
            .get(&ws)
            .and_then(|c| c.root.as_ref())
            .map(|root| {
                let mut ids = std::collections::HashSet::new();
                collect_tab_ids(root, &mut ids);
                ids
            })
            .unwrap_or_default();
        if let Some(h) = self.nav_history.get_mut(&ws) {
            h.retain(|t| live.contains(&t));
        }
    }

    fn try_activate_nav_tab(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let loc = self
            .contents
            .get(&ws)
            .and_then(|c| c.root.as_ref())
            .and_then(|r| r.find_tab(tab));
        let Some((pane, idx)) = loc else {
            return false;
        };
        self.nav_suppress = true;
        self.activate_tab_in_pane(pane, idx, window, cx);
        self.nav_suppress = false;
        true
    }
}

fn collect_tab_ids(node: &LiveNode, out: &mut std::collections::HashSet<TabId>) {
    match node {
        LiveNode::Leaf(leaf) => {
            for t in &leaf.tabs {
                out.insert(t.id());
            }
        }
        LiveNode::Split { first, second, .. } => {
            collect_tab_ids(first, out);
            collect_tab_ids(second, out);
        }
    }
}
