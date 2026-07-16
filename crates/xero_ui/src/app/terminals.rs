use super::*;

impl XeroApp {
    /// Add another terminal tab to the active workspace.
    pub(crate) fn add_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(root) = self.workspace_root(id) else {
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
    /// active. Closing the last tab leaves an empty panel (⌘N).
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

    /// Shell process died: restyle tab, then maybe auto-close per this tab's policy.
    pub(super) fn on_terminal_exited(
        &mut self,
        view: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        self.schedule_terminal_auto_close(view, cx);
    }

    /// Per-tab on-exit policy changed while already exited: reschedule or cancel.
    pub(super) fn on_terminal_auto_close_changed(
        &mut self,
        view: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) {
        self.schedule_terminal_auto_close(view, cx);
    }

    /// Close after this tab's `auto_close` delay, if any. Token cancels stale tasks.
    fn schedule_terminal_auto_close(&mut self, view: Entity<TerminalView>, cx: &mut Context<Self>) {
        let (delay, token) = {
            let term = view.read(cx);
            if !term.is_exited() {
                return;
            }
            let Some(delay) = term.auto_close().delay() else {
                return;
            };
            (delay, term.auto_close_token())
        };
        cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |this, cx| {
                this.close_terminal_entity_if_still_due(&view, token, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Auto-close path: drop a terminal by entity only if still exited and policy token matches.
    pub(super) fn close_terminal_entity_if_still_due(
        &mut self,
        view: &Entity<TerminalView>,
        token: u64,
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
        let term = view.read(cx);
        if !term.is_exited() || term.auto_close_token() != token {
            return;
        }
        if term.auto_close().delay().is_none() {
            return;
        }
        let _ = self.remove_terminal_at(id, index, cx);
        cx.notify();
    }

    /// Remove tab at index; returns the survivor to focus, if any.
    fn remove_terminal_at(
        &mut self,
        id: WorkspaceId,
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
            return None;
        }
        fix_active_after_remove(&mut stack.active, index, stack.tabs.len());
        Some(stack.tabs[stack.active].clone())
    }
}

fn fix_active_after_remove(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}
