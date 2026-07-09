use super::*;

impl XeroApp {
    pub(crate) fn activate_stream(&mut self, id: StreamId, cx: &mut Context<Self>) {
        let Some(root) = self.stream_root(id) else {
            return;
        };
        self.active = Some(id);
        self.finder = None;
        if let Some(stream) = self.streams.get(&id) {
            let layout = &stream.session.layout;
            self.terminal_collapsed = !layout.terminal_visible;
            self.editor_collapsed = !layout.editor_visible;
            self.sidebar_collapsed = !layout.sidebar_visible;
        }
        if !self.terminals.contains_key(&id) {
            let terminal = self.spawn_terminal(root, id, cx);
            self.terminals.insert(
                id,
                TerminalStack {
                    tabs: vec![terminal],
                    active: 0,
                },
            );
        }
        self.persist_active();
        cx.notify();
    }

    /// Create a terminal for `stream` at `root` and wire its bell/interaction
    /// events to the stream's attention flag.
    pub(super) fn spawn_terminal(
        &mut self,
        root: PathBuf,
        stream: StreamId,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let env = self.terminal_env();
        let terminal = cx.new(|cx| TerminalView::new(Some(root), env, cx));
        self._bell_subs.push(
            cx.subscribe(&terminal, move |this, _view, event, cx| match event {
                TerminalEvent::Bell | TerminalEvent::Finished => this.flag_attention(stream, cx),
                TerminalEvent::Interacted => this.clear_attention(stream, cx),
                TerminalEvent::Exited => cx.notify(),
                TerminalEvent::OpenPath(path) => this.open_editor(path.clone(), true, cx),
            }),
        );
        terminal
    }

    /// The stream's agent rang the bell (or went quiet): flag it in the sidebar,
    /// even when active/focused. The flag stays until the user acts in that
    /// terminal (click/type/scroll) or switches to the stream.
    fn flag_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.attention.insert(id) {
            cx.notify();
        }
    }

    fn clear_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.attention.remove(&id) {
            cx.notify();
        }
    }

    pub(crate) fn needs_attention(&self, id: StreamId) -> bool {
        self.attention.contains(&id)
    }

    /// Switch to a stream and focus its terminal, so keyboard focus lands
    /// somewhere definite on every switch (called from the sidebar click).
    pub(crate) fn select_stream(
        &mut self,
        id: StreamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_stream(id, cx);
        // Opening the stream means the user is now looking at its terminal, so
        // any pending attention mark is answered.
        self.clear_attention(id, cx);
        if let Some(terminal) = self.active_terminal() {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    pub(crate) fn add_stream(&mut self, workspace: WorkspaceId, cx: &mut Context<Self>) {
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        let stream = Stream::new(format!("stream {}", record.streams.len() + 1));
        let stream_id = stream.id;
        record.streams.push(stream_id);
        self.streams.insert(stream_id, stream.clone());
        save_session(workspace, &stream, "add_stream");
        save_registry(&self.registry, "add_stream");
        self.activate_stream(stream_id, cx);
    }

    /// Move `dragged` to `target`'s position within their shared workspace.
    /// No-op across workspaces (drag reorders within one workspace only).
    pub(crate) fn reorder_stream(
        &mut self,
        dragged: StreamId,
        target: StreamId,
        cx: &mut Context<Self>,
    ) {
        if dragged == target {
            return;
        }
        let Some(workspace) = self.workspace_of(target).map(|w| w.id) else {
            return;
        };
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        let Some(from) = record.streams.iter().position(|&s| s == dragged) else {
            return;
        };
        record.streams.remove(from);
        let to = record
            .streams
            .iter()
            .position(|&s| s == target)
            .unwrap_or(record.streams.len());
        record.streams.insert(to, dragged);
        save_registry(&self.registry, "reorder_stream");
        cx.notify();
    }

    /// Close a stream: drop its terminals/editors, delete its session, and remove
    /// it from its workspace. Refuses to remove a workspace's last stream.
    pub(crate) fn close_stream(
        &mut self,
        id: StreamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.workspace_of(id).map(|w| w.id) else {
            return;
        };
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        if record.streams.len() <= 1 {
            return;
        }
        record.streams.retain(|&stream| stream != id);
        let fallback = record.streams.first().copied();
        self.streams.remove(&id);
        self.terminals.remove(&id);
        self.editors.remove(&id);
        self.attention.remove(&id);
        delete_session(workspace, id, "close_stream");
        save_registry(&self.registry, "close_stream");
        if self.active == Some(id) {
            self.active = None;
            if let Some(next) = fallback {
                self.select_stream(next, window, cx);
            }
        }
        cx.notify();
    }

    /// Begin renaming a stream: open an inline field seeded with its name.
    pub(crate) fn start_rename(&mut self, id: StreamId, cx: &mut Context<Self>) {
        let name = self.stream_name(id).to_string();
        let field = cx.new(|cx| RenameView::new(name, cx));
        self._rename_sub = Some(
            cx.subscribe(&field, move |this, _field, event, cx| match event {
                RenameEvent::Committed(name) => this.apply_rename(id, name.clone(), cx),
                RenameEvent::Cancelled => this.cancel_rename(cx),
            }),
        );
        self.renaming = Some((id, field));
        cx.notify();
    }

    /// The inline rename field for `id`, if that stream is being renamed.
    pub(crate) fn rename_field(&self, id: StreamId) -> Option<Entity<RenameView>> {
        self.renaming
            .as_ref()
            .filter(|(target, _)| *target == id)
            .map(|(_, field)| field.clone())
    }

    fn apply_rename(&mut self, id: StreamId, name: String, cx: &mut Context<Self>) {
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.name = name;
            if let Some(workspace) = self.workspace_of(id).map(|w| w.id) {
                save_session(workspace, &self.streams[&id], "apply_rename");
            }
        }
        self.cancel_rename(cx);
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming = None;
        self._rename_sub = None;
        cx.notify();
    }
}
