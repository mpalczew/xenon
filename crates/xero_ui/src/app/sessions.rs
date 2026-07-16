//! Workspace activation, terminals, attention, and rename.

use super::*;

impl XeroApp {
    pub(crate) fn activate_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.registry.workspace(id).is_none() {
            return;
        }
        let Some(root) = self.workspace_root(id) else {
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
        if let Some(session) = self.sessions.get(&id) {
            self.apply_layout(session.layout);
        } else {
            let session = SessionState {
                layout: self.current_layout(),
                ..Default::default()
            };
            self.apply_layout(session.layout);
            self.sessions.insert(id, session);
            save_session(id, &self.sessions[&id], "activate_workspace default");
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

    /// Create a terminal for `workspace` at `root` and wire its bell/interaction
    /// events. Attention is resolved by which workspace owns the terminal view.
    pub(super) fn spawn_terminal(
        &mut self,
        root: PathBuf,
        _workspace: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let env = self.terminal_env();
        let terminal = cx.new(|cx| TerminalView::new(Some(root), env, cx));
        self._bell_subs
            .push(cx.subscribe(&terminal, move |this, view, event, cx| {
                let owner = this.workspace_of_terminal(&view);
                match event {
                    TerminalEvent::Bell | TerminalEvent::Finished => {
                        if let Some(workspace) = owner {
                            this.flag_attention(workspace, cx);
                        }
                    }
                    TerminalEvent::Interacted => {
                        if let Some(workspace) = owner {
                            this.clear_attention(workspace, cx);
                        }
                    }
                    TerminalEvent::Exited => this.on_terminal_exited(view.clone(), cx),
                    TerminalEvent::AutoCloseChanged => {
                        this.on_terminal_auto_close_changed(view.clone(), cx)
                    }
                    TerminalEvent::OpenPath(path) => this.open_editor(path.clone(), true, cx),
                    TerminalEvent::ResolvePath(token) => this.resolve_clicked(token.clone(), cx),
                }
            }));
        terminal
    }

    /// Which workspace currently holds this terminal tab (if any).
    fn workspace_of_terminal(&self, view: &Entity<TerminalView>) -> Option<WorkspaceId> {
        self.terminals
            .iter()
            .find_map(|(id, stack)| stack.tabs.iter().any(|tab| tab == view).then_some(*id))
    }

    /// A cmd-clicked token in the terminal did not resolve to a file on disk.
    /// Fuzzy-match it against the workspace index: open the one clear match, else
    /// open cmd-p prefilled with the name to disambiguate.
    fn resolve_clicked(&mut self, token: String, cx: &mut Context<Self>) {
        let Some(root) = self.active.and_then(|id| self.workspace_root(id)) else {
            return;
        };
        self.reindex(root.clone(), false, cx);
        let basename = Path::new(&token)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| token.clone());
        let Some(index) = self.file_indexes.get(&root).cloned() else {
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

    fn flag_attention(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.attention.insert(id) {
            cx.notify();
        }
    }

    pub(crate) fn clear_attention(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.attention.remove(&id) {
            cx.notify();
        }
    }

    pub(crate) fn needs_attention(&self, id: WorkspaceId) -> bool {
        self.attention.contains(&id)
    }

    /// Switch to a workspace and focus its terminal.
    pub(crate) fn select_workspace(
        &mut self,
        id: WorkspaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_workspace(id, cx);
        self.clear_attention(id, cx);
        if let Some(terminal) = self.active_terminal() {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    /// Begin renaming a workspace display name (root path stays put).
    pub(crate) fn start_rename_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        let name = self
            .registry
            .workspace(id)
            .map(|w| w.name.clone())
            .unwrap_or_default();
        self.begin_rename(RenameTarget::Workspace(id), name, cx);
    }

    fn begin_rename(&mut self, target: RenameTarget, name: String, cx: &mut Context<Self>) {
        let field = cx.new(|cx| RenameView::new(name, cx));
        self._rename_sub = Some(
            cx.subscribe(&field, move |this, _field, event, cx| match event {
                RenameEvent::Committed(name) => this.apply_rename(target, name.clone(), cx),
                RenameEvent::Cancelled => this.cancel_rename(cx),
            }),
        );
        self.renaming = Some((target, field));
        cx.notify();
    }

    /// The inline rename field for workspace `id`, if that workspace is renaming.
    pub(crate) fn rename_workspace_field(&self, id: WorkspaceId) -> Option<Entity<RenameView>> {
        self.renaming
            .as_ref()
            .filter(|(target, _)| *target == RenameTarget::Workspace(id))
            .map(|(_, field)| field.clone())
    }

    fn apply_rename(&mut self, target: RenameTarget, name: String, cx: &mut Context<Self>) {
        match target {
            RenameTarget::Workspace(id) => {
                if let Some(workspace) = self.registry.workspace_mut(id) {
                    workspace.name = name;
                    save_registry(&self.registry, "rename_workspace");
                }
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
