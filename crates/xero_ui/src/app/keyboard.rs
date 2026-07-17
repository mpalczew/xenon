//! Keyboard-first navigation: pane focus, workspace cycle, tabs, closes.

use super::*;
use xero_core::WorkspaceId;

impl XeroApp {
    /// Focus the terminal panel (opens it if needed).
    pub(super) fn focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser_focused = false;
        self.terminal_collapsed = false;
        self.deferred.last_font_pane = FontPane::Terminal;
        self.ensure_terminal(cx);
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        if let Some(terminal) = self.active_terminal() {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    /// Focus the editor panel (opens it if needed).
    pub(super) fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser_focused = false;
        self.editor_collapsed = false;
        self.deferred.last_font_pane = FontPane::Editor;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        if let Some(editor) = self.active_editor() {
            editor.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    /// Focus the file tree in the workspace panel (opens it if needed).
    pub(super) fn focus_browser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_collapsed = false;
        if !self.file_browser.is_open() {
            self.show_browser(cx);
        }
        self.browser_focused = true;
        self.file_browser.ensure_cursor();
        // Keep app focus so tree keys hit XeroApp, not a dead editor handle.
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Cycle focus: terminal → editor → file tree (when open) → terminal.
    pub(super) fn focus_next_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let order = self.pane_cycle_order();
        if order.is_empty() {
            return;
        }
        let current = self.current_focus_slot(window, cx);
        let idx = order
            .iter()
            .position(|p| Some(*p) == current)
            .map(|i| (i + 1) % order.len())
            .unwrap_or(0);
        self.focus_pane_slot(order[idx], window, cx);
    }

    fn pane_cycle_order(&self) -> Vec<FocusPane> {
        let mut order = Vec::new();
        if self.terminal_visible() {
            order.push(FocusPane::Terminal);
        }
        if self.editor_visible() {
            order.push(FocusPane::Editor);
        }
        if self.is_browsing() {
            order.push(FocusPane::Browser);
        }
        order
    }

    fn current_focus_slot(&self, window: &Window, cx: &Context<Self>) -> Option<FocusPane> {
        if self.browser_focused {
            return Some(FocusPane::Browser);
        }
        self.focused_pane(window, cx)
    }

    fn focus_pane_slot(&mut self, pane: FocusPane, window: &mut Window, cx: &mut Context<Self>) {
        match pane {
            FocusPane::Terminal => self.focus_terminal(window, cx),
            FocusPane::Editor => self.focus_editor(window, cx),
            FocusPane::Browser => self.focus_browser(window, cx),
        }
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

    /// Next tab in the focused surface (terminal or editor).
    pub(super) fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, window, cx);
    }

    pub(super) fn prev_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(-1, window, cx);
    }

    fn step_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        match self.focused_pane(window, cx) {
            Some(FocusPane::Terminal) | None if self.terminal_visible() => {
                let Some(stack) = self.terminal_stack() else {
                    return;
                };
                if stack.tabs.is_empty() {
                    return;
                }
                let len = stack.tabs.len() as isize;
                let next = (stack.active as isize + delta).rem_euclid(len) as usize;
                self.activate_terminal_tab(next, window, cx);
            }
            _ => {
                let Some(stack) = self.editor_stack() else {
                    return;
                };
                if stack.tabs.is_empty() {
                    return;
                }
                let len = stack.tabs.len() as isize;
                let next = (stack.active as isize + delta).rem_euclid(len) as usize;
                self.activate_tab(next, cx);
                if let Some(editor) = self.active_editor() {
                    editor.read(cx).focus_handle(cx).focus(window, cx);
                }
            }
        }
    }

    /// ⌘W: close terminal tab if terminal focused, else editor tab.
    pub(super) fn close_focused_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.focused_pane(window, cx), Some(FocusPane::Terminal))
            && let Some(stack) = self.terminal_stack()
        {
            let index = stack.active;
            self.close_terminal_tab(index, window, cx);
            return;
        }
        self.close_editor(window, cx);
    }
}
