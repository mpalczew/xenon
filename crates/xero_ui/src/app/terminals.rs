use super::*;

impl XeroApp {
    /// Add another terminal tab to the active stream.
    pub(crate) fn add_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(root) = self.stream_root(id) else {
            return;
        };
        let terminal = self.spawn_terminal(root, id, cx);
        let stack = self.terminals.entry(id).or_default();
        stack.tabs.push(terminal);
        stack.active = stack.tabs.len() - 1;
        self.terminal_collapsed = false;
        self.save_layout(id);
        cx.notify();
    }

    /// Cmd-N / File → New Terminal: new tab and focus it.
    pub(crate) fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_terminal(cx);
        if let Some(terminal) = self.active_terminal() {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    /// Cmd-Shift-N / File → New Stream: parallel work unit in active workspace.
    pub(crate) fn new_stream(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self
            .active
            .and_then(|id| self.workspace_of(id).map(|w| w.id))
            .or_else(|| self.registry.workspaces.first().map(|w| w.id));
        let Some(workspace) = workspace else {
            return;
        };
        let Some(id) = self.create_stream(workspace) else {
            return;
        };
        self.select_stream(id, window, cx);
    }

    pub(crate) fn terminal_stack(&self) -> Option<&TerminalStack> {
        self.active.and_then(|id| self.terminals.get(&id))
    }

    pub(super) fn active_terminal(&self) -> Option<Entity<TerminalView>> {
        self.terminal_stack()
            .and_then(|s| s.tabs.get(s.active))
            .cloned()
    }

    pub(crate) fn activate_terminal_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let terminal = {
            let Some(stack) = self.terminals.get_mut(&id) else {
                return;
            };
            if index >= stack.tabs.len() {
                return;
            }
            stack.active = index;
            stack.tabs[index].clone()
        };
        terminal.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Close a terminal tab and move focus to the terminal that becomes
    /// active. Closing the last tab collapses the terminal panel.
    pub(crate) fn close_terminal_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let survivor = self.remove_terminal_at(id, index, cx);
        if let Some(terminal) = survivor {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    /// Shell process died: restyle tab, then maybe auto-close after the setting delay.
    pub(super) fn on_terminal_exited(
        &mut self,
        view: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let Some(delay) = xero_settings::terminal_auto_close(cx).delay() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |this, cx| {
                this.close_terminal_entity(&view, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Auto-close path: drop a terminal by entity (index may have shifted).
    pub(super) fn close_terminal_entity(
        &mut self,
        view: &Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        let Some((id, index)) = self.terminals.iter().find_map(|(id, stack)| {
            stack
                .tabs
                .iter()
                .position(|tab| tab == view)
                .map(|index| (*id, index))
        }) else {
            return;
        };
        // Only auto-close if still exited (user may have replaced the tab).
        if !view.read(cx).is_exited() {
            return;
        }
        let _ = self.remove_terminal_at(id, index, cx);
        cx.notify();
    }

    /// Remove tab at index; returns the survivor to focus, if any.
    fn remove_terminal_at(
        &mut self,
        id: StreamId,
        index: usize,
        _cx: &mut Context<Self>,
    ) -> Option<Entity<TerminalView>> {
        let stack = self.terminals.get_mut(&id)?;
        if index >= stack.tabs.len() {
            return None;
        }
        stack.tabs.remove(index);
        if stack.tabs.is_empty() {
            self.terminals.remove(&id);
            if self.active == Some(id) {
                self.terminal_collapsed = true;
                self.save_layout(id);
            }
            return None;
        }
        fix_active_after_remove(&mut stack.active, index, stack.tabs.len());
        Some(stack.tabs[stack.active].clone())
    }

    /// Move a terminal tab to another stream in the same workspace, then switch
    /// to that stream and focus the moved tab.
    pub(crate) fn move_terminal_tab(
        &mut self,
        tab: TabMove,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab.from == tab.to || !self.same_workspace(tab.from, tab.to) {
            return;
        }
        let Some(terminal) = self.take_terminal_tab(tab.from, tab.index) else {
            return;
        };
        let stack = self.terminals.entry(tab.to).or_default();
        stack.tabs.push(terminal.clone());
        stack.active = stack.tabs.len() - 1;
        self.terminal_collapsed = false;
        self.save_layout(tab.to);
        self.activate_stream(tab.to, cx);
        self.clear_attention(tab.to, cx);
        terminal.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(crate) fn move_terminal_tab_to_new_stream(
        &mut self,
        from: StreamId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.workspace_of(from).map(|w| w.id) else {
            return;
        };
        let Some(to) = self.create_stream(workspace) else {
            return;
        };
        self.move_terminal_tab(TabMove { from, index, to }, window, cx);
    }

    fn take_terminal_tab(&mut self, from: StreamId, index: usize) -> Option<Entity<TerminalView>> {
        let (terminal, emptied) = {
            let stack = self.terminals.get_mut(&from)?;
            if index >= stack.tabs.len() {
                return None;
            }
            let terminal = stack.tabs.remove(index);
            let emptied = stack.tabs.is_empty();
            if !emptied {
                fix_active_after_remove(&mut stack.active, index, stack.tabs.len());
            }
            (terminal, emptied)
        };
        if emptied {
            self.terminals.remove(&from);
            // Leave the stream; panel collapse only if still viewing it.
            if self.active == Some(from) {
                self.terminal_collapsed = true;
                self.save_layout(from);
            }
        }
        Some(terminal)
    }
}

fn fix_active_after_remove(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}
