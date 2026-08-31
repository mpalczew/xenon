//! Keyboard-first navigation: pane focus, workspace cycle, tabs, closes.

use super::*;
use xenon_core::{PaneId, SplitAxis, WorkspaceId};

/// Which tree target should own keys / ⌘W / tab cycle.
///
/// GPUI surface focus wins over a stale Files flag or the persisted leaf pointer.
/// Those two are fallbacks for chrome clicks (no surface owns GPUI).
pub(super) fn resolve_focus_target(
    gpui_leaf: Option<PaneId>,
    browser_focused: bool,
    session_leaf: Option<PaneId>,
) -> Option<FocusTarget> {
    if let Some(pane) = gpui_leaf {
        return Some(FocusTarget::Leaf(pane));
    }
    if browser_focused {
        return Some(FocusTarget::Browser);
    }
    session_leaf.map(FocusTarget::Leaf)
}

pub(crate) fn tab_has_gpui_focus(tab: &LiveTab, window: &Window, cx: &App) -> bool {
    match tab {
        LiveTab::Terminal { view, .. } => {
            view.read(cx).focus_handle(cx).contains_focused(window, cx)
        }
        LiveTab::Editor { view, .. } => view.read(cx).focus_handle(cx).contains_focused(window, cx),
    }
}

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

    /// Leaf whose active tab owns GPUI keyboard focus, if any.
    pub(super) fn leaf_with_gpui_focus(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<PaneId> {
        let root = self.active_content()?.root.as_ref()?;
        for pane in root.leaf_ids() {
            if let Some(leaf) = root.find_leaf(pane)
                && leaf
                    .active_tab()
                    .is_some_and(|tab| tab_has_gpui_focus(tab, window, cx))
            {
                return Some(pane);
            }
        }
        None
    }

    fn current_focus_target(&self, window: &Window, cx: &Context<Self>) -> Option<FocusTarget> {
        resolve_focus_target(
            self.leaf_with_gpui_focus(window, cx),
            self.browser_focused,
            self.active_content().and_then(|c| c.focused),
        )
    }

    /// Point the session leaf at the GPUI-focused surface so ⌘W / split / ⌘N
    /// match the pane the user is actually typing in.
    pub(super) fn follow_gpui_leaf(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(pane) = self.leaf_with_gpui_focus(window, cx) else {
            return;
        };
        self.adopt_focused_pane(pane, cx);
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
        self.follow_gpui_leaf(window, cx);
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
        self.follow_gpui_leaf(window, cx);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FocusTarget {
    Leaf(PaneId),
    Browser,
}

#[cfg(test)]
mod tests {
    use super::{FocusTarget, resolve_focus_target};
    use xenon_core::PaneId;

    #[test]
    fn resolve_focus_target_prefers_gpui_then_browser_then_session() {
        let gpui = PaneId(2);
        let session = PaneId(1);
        assert_eq!(
            resolve_focus_target(Some(gpui), true, Some(session)),
            Some(FocusTarget::Leaf(gpui))
        );
        assert_eq!(
            resolve_focus_target(None, true, Some(session)),
            Some(FocusTarget::Browser)
        );
        assert_eq!(
            resolve_focus_target(None, false, Some(session)),
            Some(FocusTarget::Leaf(session))
        );
        assert_eq!(resolve_focus_target(None, false, None), None);
    }
}
