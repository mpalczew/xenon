//! Workspace activation, terminals, and rename.

use super::*;
use xenon_core::TabState;

/// Terminal path clicks that are view-first (not edit-first) should hand off
/// to the OS default app instead of opening a Xenon editor tab.
fn prefer_system_open(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("html" | "htm" | "pdf")
    )
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
            if self.worklist_capture_visible == Some(prev) {
                self.hide_worklist_capture(cx);
                self.deferred.pending_focus = self.deferred.restore_pane.take();
            }
        }
        self.active = Some(id);
        self.finder = None;
        self.close_theme_picker(cx);

        let session = self.ensure_live_content(id, &root, cx);
        self.apply_sidebar(&session);
        self.reindex(root, false, cx);
        self.persist_active();
        self.nav_seed_active(id, cx);
        self.save_layout(id);
        cx.notify();
    }

    /// Build the workspace's live tabs (terminals spawn) if this process has
    /// not yet, without switching to it. Returns its session layout.
    pub(crate) fn ensure_live_content(
        &mut self,
        id: WorkspaceId,
        root: &Path,
        cx: &mut Context<Self>,
    ) -> SessionState {
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
        if !self.contents.contains_key(&id) {
            let live = self.build_live_from_session(&session, root, id, cx);
            self.seed_recent_from_content(id, &live);
            self.contents.insert(id, live);
        }
        session
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
                        TabState::Terminal {
                            id, remote_session, ..
                        } => {
                            if let Some(session) = remote_session {
                                self.services
                                    .ssh_sessions
                                    .insert((workspace, *id), session.clone());
                            }
                            let view =
                                self.spawn_terminal_for(root.to_path_buf(), (workspace, *id), cx);
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
                            match self.build_workspace_editor(abs.clone(), root, false, cx) {
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
                if tabs.is_empty() && !leaf.parked {
                    // Keep leaf valid with a terminal.
                    let view = self.spawn_terminal_for(
                        root.to_path_buf(),
                        (workspace, TabId(leaf.id.0.saturating_mul(1000) + 1)),
                        cx,
                    );
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
                    parked: leaf.parked,
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
        workspace: WorkspaceId,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let tab = self
            .contents
            .get(&workspace)
            .map(|content| content.next_tab_id())
            .unwrap_or(TabId(1));
        self.services.ssh_sessions.remove(&(workspace, tab));
        self.spawn_terminal_for(root, (workspace, tab), cx)
    }

    pub(super) fn spawn_terminal_for(
        &mut self,
        root: PathBuf,
        owner: (WorkspaceId, TabId),
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let env = self.terminal_env(&root);
        let terminal = if let Some(ssh) = self.ssh_for_root(&root) {
            let session = self
                .services
                .ssh_sessions
                .entry(owner)
                .or_insert_with(|| format!("xenon-{}", WorkspaceId::new()))
                .clone();
            match ssh.terminal_command(&session) {
                Ok(command) => cx.new(|cx| TerminalView::ssh(command, cx)),
                Err(error) => {
                    self.show_toast(
                        super::toasts::failed("Couldn’t open SSH terminal", error),
                        cx,
                    );
                    cx.new(|cx| TerminalView::ssh("exit 1".into(), cx))
                }
            }
        } else {
            cx.new(|cx| TerminalView::new(Some(root), env, cx))
        };
        self._bell_subs
            .push(cx.subscribe(&terminal, move |this, view, event, cx| {
                let owner = this.locate_terminal(&view);
                match event {
                    TerminalEvent::Bell => {
                        if let Some((workspace, tab)) = owner {
                            this.flag_attention(workspace, tab, AttentionReason::Bell, cx);
                            this.refresh_terminal_status(cx);
                        }
                    }
                    TerminalEvent::Working => {
                        let title = view.read(cx).title(cx);
                        this.maybe_offer_skill(&title, cx);
                        this.refresh_terminal_status(cx);
                    }
                    TerminalEvent::Finished => {
                        if let Some((workspace, tab)) = owner {
                            this.flag_attention(workspace, tab, AttentionReason::IdleSettled, cx);
                        }
                        // Sibling may still be working; still repaint this tab's pip.
                        this.refresh_terminal_status(cx);
                    }
                    TerminalEvent::Interacted => {
                        if let Some((workspace, tab)) = owner {
                            this.clear_tab_attention(workspace, tab, cx);
                            this.adopt_tab_as_focused(workspace, tab, cx);
                        }
                        this.refresh_terminal_status(cx);
                    }
                    TerminalEvent::Exited => {
                        this.refresh_terminal_status(cx);
                        this.on_terminal_exited(view.clone(), cx);
                    }
                    TerminalEvent::AutoCloseChanged => {
                        this.on_terminal_auto_close_changed(view.clone(), cx)
                    }
                    TerminalEvent::OpenPath(path) => {
                        if let Some(path) = this.terminal_workspace_path(owner, path, cx) {
                            this.open_terminal_path(path, cx);
                        }
                    }
                    TerminalEvent::OpenInEditor(path) => {
                        if let Some(path) = this.terminal_workspace_path(owner, path, cx) {
                            this.open_editor(path, true, cx);
                        }
                    }
                    TerminalEvent::ResolvePath(token) => this.resolve_clicked(token.clone(), cx),
                }
            }));
        terminal
    }

    /// Terminal cmd-click on a path: browser-native types go to the system
    /// default app; everything else opens as an editor tab.
    fn open_terminal_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.starts_with("/__xenon_ssh__") && prefer_system_open(&path) {
            cx.open_with_system(&path);
            return;
        }
        self.open_editor(path, true, cx);
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
            self.open_terminal_path(root.join(rel), cx);
        } else if exact.is_empty()
            && let Some(rel) = unique_file()
        {
            self.open_terminal_path(root.join(rel), cx);
        } else {
            self.deferred.pending_palette_query = Some(basename);
            cx.notify();
        }
    }

    pub(crate) fn select_workspace(
        &mut self,
        id: WorkspaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_workspace(id, cx);
        self.dismiss_viewed_terminal(cx);
        self.focus_workspace_leaf(Some(window), cx);
    }

    pub(crate) fn start_rename_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        let name = self
            .registry
            .workspace(id)
            .map(|w| w.name.clone())
            .unwrap_or_default();
        self.begin_rename(RenameTarget::Workspace(id), name, cx);
    }

    pub(crate) fn begin_rename(
        &mut self,
        target: RenameTarget,
        name: String,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| RenameView::new(name, cx));
        let subscribed_target = target.clone();
        self._rename_sub = Some(
            cx.subscribe(&field, move |this, _field, event, cx| match event {
                RenameEvent::Committed(name) => {
                    this.apply_rename(subscribed_target.clone(), name.clone(), cx)
                }
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

    pub(crate) fn rename_file_field(&self, path: &Path) -> Option<Entity<RenameView>> {
        self.renaming
            .as_ref()
            .filter(|(target, _)| {
                matches!(target, RenameTarget::File { path: target_path, .. } if target_path == path)
            })
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
            RenameTarget::File { path, .. } => {
                if path.starts_with("/__xenon_ssh__") {
                    self.show_toast(
                        super::toasts::failed(
                            "Rename the remote file in a terminal",
                            "Remote tree renaming is unavailable",
                        ),
                        cx,
                    );
                    self.cancel_rename(cx);
                    return;
                }
                let Some(parent) = path.parent() else { return };
                let destination = parent.join(&name);
                if destination != *path {
                    match rename_file(&path, &destination) {
                        Ok(()) => {
                            if let Some(root) = self.active.and_then(|id| self.workspace_root(id)) {
                                self.reindex(root, true, cx);
                            }
                        }
                        Err(error) => self
                            .show_toast(super::toasts::failed("Couldn’t rename that", error), cx),
                    }
                }
            }
        }
        self.cancel_rename(cx);
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if let Some((
            RenameTarget::File {
                path,
                created: true,
            },
            _,
        )) = &self.renaming
            && path.exists()
        {
            let _ = std::fs::remove_file(path);
        }
        self.renaming = None;
        self._rename_sub = None;
        cx.notify();
    }
}

fn rename_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    if to.exists() {
        anyhow::bail!("{} already exists", file_name(to));
    }
    std::fs::rename(from, to)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prefer_system_open;
    use std::path::Path;

    #[test]
    fn prefer_system_open_html_and_pdf() {
        assert!(prefer_system_open(Path::new("tmp/bakeoff.html")));
        assert!(prefer_system_open(Path::new("report.HTM")));
        assert!(prefer_system_open(Path::new("/abs/doc.PDF")));
    }

    #[test]
    fn prefer_system_open_source_stays_in_editor() {
        assert!(!prefer_system_open(Path::new("src/main.rs")));
        assert!(!prefer_system_open(Path::new("index.md")));
        assert!(!prefer_system_open(Path::new("styles.css")));
        assert!(!prefer_system_open(Path::new("noext")));
    }
}
