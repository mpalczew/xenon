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
        let survivor = {
            let Some(stack) = self.terminals.get_mut(&id) else {
                return;
            };
            if index >= stack.tabs.len() {
                return;
            }
            stack.tabs.remove(index);
            if stack.tabs.is_empty() {
                None
            } else {
                stack.active = stack.active.min(stack.tabs.len() - 1);
                Some(stack.tabs[stack.active].clone())
            }
        };
        match survivor {
            Some(terminal) => terminal.read(cx).focus_handle(cx).focus(window, cx),
            None => {
                self.terminals.remove(&id);
                self.terminal_collapsed = true;
                self.save_layout(id);
            }
        }
        cx.notify();
    }
}
