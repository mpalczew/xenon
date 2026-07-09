use super::*;

impl XeroApp {
    pub(crate) fn sidebar_visible(&self) -> bool {
        !self.sidebar_collapsed
    }

    pub(crate) fn terminal_visible(&self) -> bool {
        !self.terminal_collapsed && self.active_terminal().is_some()
    }

    pub(crate) fn editor_visible(&self) -> bool {
        !self.editor_collapsed && self.active.is_some()
    }

    /// Write the live collapsed flags into the active stream's session and
    /// save, so pane visibility survives a stream switch or app restart.
    pub(super) fn save_layout(&mut self, id: StreamId) {
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.session.layout = Layout {
                terminal_visible: !self.terminal_collapsed,
                editor_visible: !self.editor_collapsed,
                sidebar_visible: !self.sidebar_collapsed,
            };
            if let Some(workspace) = self.workspace_of(id).map(|w| w.id) {
                save_session(workspace, &self.streams[&id], "save_layout");
            }
        }
    }

    pub(super) fn toggle_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_collapsed = !self.terminal_collapsed;
        if !self.terminal_collapsed {
            let id = self.active;
            if let Some(id) = id
                && !self.terminals.contains_key(&id)
                && let Some(root) = self.stream_root(id)
            {
                let terminal = self.spawn_terminal(root, id, cx);
                self.terminals.insert(
                    id,
                    TerminalStack {
                        tabs: vec![terminal],
                        active: 0,
                    },
                );
            }
            if let Some(terminal) = self.active_terminal() {
                terminal.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    pub(super) fn toggle_editor_panel(&mut self, cx: &mut Context<Self>) {
        self.editor_collapsed = !self.editor_collapsed;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    pub(super) fn toggle_sidebar_panel(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }
}
