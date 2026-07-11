use super::*;

impl XeroApp {
    pub(crate) fn activate_stream(&mut self, id: StreamId, cx: &mut Context<Self>) {
        let Some(root) = self.stream_root(id) else {
            return;
        };
        // Flush live widths/visibility before switching so a drag is not lost.
        if let Some(prev) = self.active
            && prev != id
        {
            self.save_layout(prev);
        }
        self.active = Some(id);
        self.finder = None;
        self.reindex(root.clone(), false, cx);
        if let Some(stream) = self.streams.get(&id) {
            self.apply_layout(stream.session.layout);
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
    /// events. Attention is resolved by which stream owns the terminal view so
    /// tabs can move between streams without rewiring.
    pub(super) fn spawn_terminal(
        &mut self,
        root: PathBuf,
        _stream: StreamId,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let env = self.terminal_env();
        let terminal = cx.new(|cx| TerminalView::new(Some(root), env, cx));
        self._bell_subs
            .push(cx.subscribe(&terminal, move |this, view, event, cx| {
                let owner = this.stream_of_terminal(&view);
                match event {
                    TerminalEvent::Bell | TerminalEvent::Finished => {
                        if let Some(stream) = owner {
                            this.flag_attention(stream, cx);
                        }
                    }
                    TerminalEvent::Interacted => {
                        if let Some(stream) = owner {
                            this.clear_attention(stream, cx);
                        }
                    }
                    TerminalEvent::Exited => cx.notify(),
                    TerminalEvent::OpenPath(path) => this.open_editor(path.clone(), true, cx),
                    TerminalEvent::ResolvePath(token) => this.resolve_clicked(token.clone(), cx),
                }
            }));
        terminal
    }

    /// Which stream currently holds this terminal tab (if any).
    fn stream_of_terminal(&self, view: &Entity<TerminalView>) -> Option<StreamId> {
        self.terminals
            .iter()
            .find_map(|(id, stack)| stack.tabs.iter().any(|tab| tab == view).then_some(*id))
    }

    /// A cmd-clicked token in the terminal did not resolve to a file on disk.
    /// Fuzzy-match it against the workspace index: open the one clear match, else
    /// open cmd-p prefilled with the name to disambiguate ("best match, else finder").
    fn resolve_clicked(&mut self, token: String, cx: &mut Context<Self>) {
        let Some(root) = self.active.and_then(|id| self.stream_root(id)) else {
            return;
        };
        self.reindex(root.clone(), false, cx);
        let basename = Path::new(&token)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| token.clone());
        let Some(index) = self.file_indexes.get(&root).cloned() else {
            // Index still building; let the prefilled palette populate when ready.
            self.pending_palette_query = Some(basename);
            cx.notify();
            return;
        };
        let results = Finder::new(index).query(&basename);
        let files = || results.iter().filter(|m| !m.is_dir);
        let exact: Vec<&std::path::PathBuf> = files()
            .filter(|m| {
                m.path.file_name().map(|n| n.to_string_lossy()) == Some(basename.as_str().into())
            })
            .map(|m| &m.path)
            .collect();
        let unique_file = || {
            files()
                .next()
                .filter(|_| files().count() == 1)
                .map(|m| &m.path)
        };
        if let Some(rel) = exact.first().copied().filter(|_| exact.len() == 1) {
            self.open_editor(root.join(rel), true, cx);
        } else if exact.is_empty()
            && let Some(rel) = unique_file()
        {
            self.open_editor(root.join(rel), true, cx);
        } else {
            self.pending_palette_query = Some(basename);
            cx.notify();
        }
    }

    /// The stream's agent rang the bell (or went quiet): flag it in the sidebar,
    /// even when active/focused. The flag stays until the user acts in that
    /// terminal (click/type/scroll) or switches to the stream.
    fn flag_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.attention.insert(id) {
            cx.notify();
        }
    }

    pub(crate) fn clear_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
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
        if let Some(stream_id) = self.create_stream(workspace) {
            self.activate_stream(stream_id, cx);
        }
    }

    /// Create a stream in `workspace` without activating it. Used by add-stream
    /// and by move-tab-to-new-stream.
    pub(crate) fn create_stream(&mut self, workspace: WorkspaceId) -> Option<StreamId> {
        // Capture before mutably borrowing the registry.
        let layout = self.current_layout();
        let record = self.registry.workspace_mut(workspace)?;
        let mut stream = Stream::new(format!("stream {}", record.streams.len() + 1));
        // New streams inherit the live layout (visibility + widths).
        stream.session.layout = layout;
        let stream_id = stream.id;
        record.streams.push(stream_id);
        self.streams.insert(stream_id, stream.clone());
        save_session(workspace, &stream, "create_stream");
        save_registry(&self.registry, "create_stream");
        Some(stream_id)
    }

    /// Other streams in the same workspace as `stream` (for move-tab menus).
    pub(crate) fn sibling_streams(&self, stream: StreamId) -> Vec<(StreamId, String)> {
        let Some(workspace) = self.workspace_of(stream) else {
            return Vec::new();
        };
        workspace
            .streams
            .iter()
            .copied()
            .filter(|&id| id != stream)
            .map(|id| (id, self.stream_name(id).to_string()))
            .collect()
    }

    pub(crate) fn same_workspace(&self, a: StreamId, b: StreamId) -> bool {
        match (self.workspace_of(a), self.workspace_of(b)) {
            (Some(wa), Some(wb)) => wa.id == wb.id,
            _ => false,
        }
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
