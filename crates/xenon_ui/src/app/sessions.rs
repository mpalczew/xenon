//! Workspace activation, terminals, attention, and rename.

use super::*;
use xenon_core::{PaneId, TabId, TabState};

/// Why a workspace attention badge is lit. Last write wins; for debug tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttentionReason {
    Bell,
    IdleSettled,
}

impl AttentionReason {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::IdleSettled => "Idle after busy output",
        }
    }
}

impl XenonApp {
    pub(crate) fn activate_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.registry.workspace(id).is_none() {
            return;
        }
        let Some(root) = self.workspace_root(id) else {
            return;
        };
        if let Some(prev) = self.active
            && prev != id
        {
            self.save_layout(prev);
        }
        self.active = Some(id);
        self.finder = None;

        let session = self.sessions.get(&id).cloned().unwrap_or_else(|| {
            let session = SessionState {
                sidebar_visible: !self.sidebar_collapsed,
                sidebar_width: xenon_core::clamp_sidebar(self.sidebar_width),
                content: Default::default(),
            };
            self.sessions.insert(id, session.clone());
            save_session(id, &session, "activate_workspace default");
            session
        });
        self.apply_sidebar(&session);

        // Rebuild live content if missing (first activate this process).
        if !self.contents.contains_key(&id) {
            let live = self.build_live_from_session(&session, &root, id, cx);
            self.seed_recent_from_content(id, &live);
            self.contents.insert(id, live);
        }
        if self.contents.get(&id).is_some_and(|c| c.is_empty()) {
            let terminal = self.spawn_terminal(root.clone(), id, cx);
            let pane = PaneId(1);
            let tab = LiveTab::Terminal {
                id: TabId(1),
                view: terminal,
            };
            self.contents.insert(
                id,
                LiveContent {
                    root: Some(LiveNode::Leaf(LiveLeaf {
                        id: pane,
                        tabs: vec![tab],
                        active: 0,
                    })),
                    focused: Some(pane),
                },
            );
            self.save_layout(id);
        }

        self.reindex(root, false, cx);
        self.persist_active();
        cx.notify();
    }

    /// Seed MRU from restored tabs so cmd-p ranks open files before cold ones.
    /// Active editor is touched last so it ranks most-recent.
    fn seed_recent_from_content(&mut self, workspace: WorkspaceId, content: &LiveContent) {
        let mut paths = Vec::new();
        let mut active_path = None;
        if let Some(root) = &content.root {
            for pane in root.leaf_ids() {
                let Some(leaf) = root.find_leaf(pane) else {
                    continue;
                };
                for (i, tab) in leaf.tabs.iter().enumerate() {
                    let Some(path) = tab.editor_path() else {
                        continue;
                    };
                    if content.focused == Some(pane) && i == leaf.active {
                        active_path = Some(path.to_path_buf());
                    } else {
                        paths.push(path.to_path_buf());
                    }
                }
            }
        }
        for path in paths {
            self.touch_recent_file(workspace, &path);
        }
        if let Some(path) = active_path {
            self.touch_recent_file(workspace, &path);
        }
    }

    fn build_live_from_session(
        &mut self,
        session: &SessionState,
        root: &Path,
        workspace: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> LiveContent {
        let Some(layout_root) = session.content.root.as_ref() else {
            return LiveContent::default();
        };
        let live_root = self.materialize_node(layout_root, root, workspace, cx);
        let mut content = LiveContent {
            root: Some(live_root),
            focused: session.content.focused,
        };
        // Repair focus
        let leaves = content.leaf_ids();
        if content.focused.is_none_or(|f| !leaves.contains(&f)) {
            content.focused = leaves.first().copied();
        }
        content
    }

    fn materialize_node(
        &mut self,
        node: &xenon_core::PaneNode,
        root: &Path,
        workspace: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> LiveNode {
        match node {
            xenon_core::PaneNode::Leaf(leaf) => {
                let mut tabs = Vec::new();
                for tab in &leaf.tabs {
                    match tab {
                        TabState::Terminal { id, cwd: _ } => {
                            let view = self.spawn_terminal(root.to_path_buf(), workspace, cx);
                            tabs.push(LiveTab::Terminal { id: *id, view });
                        }
                        TabState::Editor {
                            id,
                            path,
                            cursor: _,
                            scroll_top: _,
                        } => {
                            let abs = if path.is_absolute() {
                                path.clone()
                            } else {
                                root.join(path)
                            };
                            match EditorView::build(abs.clone(), false, cx) {
                                Ok(view) => {
                                    self.wire_editor_selection(&view, cx);
                                    tabs.push(LiveTab::Editor {
                                        id: *id,
                                        path: abs.clone(),
                                        name: file_name(&abs),
                                        view,
                                    });
                                }
                                Err(e) => log::warn!("restore editor {}: {e}", abs.display()),
                            }
                        }
                    }
                }
                if tabs.is_empty() {
                    // Keep leaf valid with a terminal.
                    let view = self.spawn_terminal(root.to_path_buf(), workspace, cx);
                    tabs.push(LiveTab::Terminal {
                        id: TabId(leaf.id.0.saturating_mul(1000) + 1),
                        view,
                    });
                }
                let active = leaf.active.min(tabs.len().saturating_sub(1));
                LiveNode::Leaf(LiveLeaf {
                    id: leaf.id,
                    tabs,
                    active,
                })
            }
            xenon_core::PaneNode::Split {
                axis,
                ratio,
                first,
                second,
            } => LiveNode::Split {
                axis: *axis,
                ratio: *ratio,
                first: Box::new(self.materialize_node(first, root, workspace, cx)),
                second: Box::new(self.materialize_node(second, root, workspace, cx)),
            },
        }
    }

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
                    TerminalEvent::Bell => {
                        if let Some(workspace) = owner {
                            this.flag_attention(workspace, AttentionReason::Bell, cx);
                        }
                    }
                    TerminalEvent::Finished => {
                        if let Some(workspace) = owner {
                            this.flag_attention(workspace, AttentionReason::IdleSettled, cx);
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

    fn workspace_of_terminal(&self, view: &Entity<TerminalView>) -> Option<WorkspaceId> {
        self.locate_terminal(view).map(|(id, _, _)| id)
    }

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
            self.deferred.pending_palette_query = Some(basename);
            cx.notify();
            return;
        };
        let recents = self
            .active
            .map(|id| self.recent_files_for(id))
            .unwrap_or_default();
        let results = Finder::new(index).query(&basename, &recents);
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
            self.deferred.pending_palette_query = Some(basename);
            cx.notify();
        }
    }

    fn flag_attention(&mut self, id: WorkspaceId, reason: AttentionReason, cx: &mut Context<Self>) {
        match self.attention.insert(id, reason) {
            Some(prev) if prev == reason => {}
            _ => cx.notify(),
        }
    }

    pub(crate) fn clear_attention(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.attention.remove(&id).is_some() {
            cx.notify();
        }
    }

    pub(crate) fn attention_reason(&self, id: WorkspaceId) -> Option<AttentionReason> {
        self.attention.get(&id).copied()
    }

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
