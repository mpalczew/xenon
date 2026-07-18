//! Keyboard-first navigation: pane focus, workspace cycle, tabs, closes.

use super::*;
use xenon_core::{SplitAxis, WorkspaceId};

impl XenonApp {
    pub(super) fn focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_or_new_terminal(window, cx);
    }

    pub(super) fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_or_reveal_editor(window, cx);
    }

    pub(super) fn focus_browser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_collapsed = false;
        if !self.file_browser.is_open() {
            self.show_browser(cx);
        }
        self.browser_focused = true;
        self.file_browser.ensure_cursor();
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Cycle focus: each leaf in tree order, then browser if open.
    pub(super) fn focus_next_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leaves = self
            .active_content()
            .map(|c| c.leaf_ids())
            .unwrap_or_default();
        let mut order: Vec<FocusTarget> = leaves.into_iter().map(FocusTarget::Leaf).collect();
        if self.is_browsing() {
            order.push(FocusTarget::Browser);
        }
        if order.is_empty() {
            return;
        }
        let current = self.current_focus_target(window, cx);
        let idx = order
            .iter()
            .position(|p| Some(*p) == current)
            .map(|i| (i + 1) % order.len())
            .unwrap_or(0);
        match order[idx] {
            FocusTarget::Leaf(pane) => self.focus_leaf_active(pane, window, cx),
            FocusTarget::Browser => self.focus_browser(window, cx),
        }
    }

    fn current_focus_target(&self, window: &Window, cx: &Context<Self>) -> Option<FocusTarget> {
        if self.browser_focused {
            return Some(FocusTarget::Browser);
        }
        // Which leaf has focused surface?
        let content = self.active_content()?;
        let root = content.root.as_ref()?;
        for pane in root.leaf_ids() {
            if let Some(leaf) = root.find_leaf(pane)
                && let Some(tab) = leaf.active_tab()
            {
                let focused = match tab {
                    LiveTab::Terminal { view, .. } => {
                        view.read(cx).focus_handle(cx).contains_focused(window, cx)
                    }
                    LiveTab::Editor { view, .. } => {
                        view.read(cx).focus_handle(cx).contains_focused(window, cx)
                    }
                };
                if focused {
                    return Some(FocusTarget::Leaf(pane));
                }
            }
        }
        content.focused.map(FocusTarget::Leaf)
    }

    pub(super) fn next_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_workspace(1, window, cx);
    }

    pub(super) fn prev_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_workspace(-1, window, cx);
    }

    fn step_workspace(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let workspaces: Vec<WorkspaceId> = self.registry.workspaces.iter().map(|w| w.id).collect();
        if workspaces.is_empty() {
            return;
        }
        let cur = self
            .active
            .and_then(|id| workspaces.iter().position(|&w| w == id))
            .unwrap_or(0);
        let len = workspaces.len() as isize;
        let next = (cur as isize + delta).rem_euclid(len) as usize;
        self.select_workspace(workspaces[next], window, cx);
    }

    pub(super) fn close_active_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        self.close_workspace(id, window, cx);
    }

    pub(super) fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, window, cx);
    }

    pub(super) fn prev_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(-1, window, cx);
    }

    fn step_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(content) = self.active_content() else {
            return;
        };
        let Some(leaf) = content.focused_leaf() else {
            return;
        };
        if leaf.tabs.is_empty() {
            return;
        }
        let pane = leaf.id;
        let len = leaf.tabs.len() as isize;
        let next = (leaf.active as isize + delta).rem_euclid(len) as usize;
        self.activate_tab_in_pane(pane, next, window, cx);
    }

    pub(super) fn close_focused_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(content) = self.active_content() else {
            return;
        };
        let Some(leaf) = content.focused_leaf() else {
            return;
        };
        let Some(tab) = leaf.active_tab() else {
            return;
        };
        let tab_id = tab.id();
        self.close_tab_id(tab_id, window, cx);
    }

    pub(super) fn split_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.do_split(SplitAxis::Horizontal, window, cx);
    }

    pub(super) fn split_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.do_split(SplitAxis::Vertical, window, cx);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusTarget {
    Leaf(xenon_core::PaneId),
    Browser,
}
