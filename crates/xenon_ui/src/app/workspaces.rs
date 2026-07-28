use super::*;
use crate::workspace_picker::{WorkspaceCandidate, WorkspacePickerEvent, WorkspacePickerView};

impl XenonApp {
    /// CLI / IPC: open a directory as a workspace, or a file in the best workspace.
    ///
    /// File rules: longest matching open/closed workspace root wins; if none,
    /// open in the current workspace (still opens even when outside the root).
    pub fn open_cli_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        if path.is_dir() {
            self.register_workspace(path, cx);
            return;
        }
        if !path.is_file() {
            log::warn!("open_cli_path: not a file or directory: {}", path.display());
            return;
        }
        if let Some(workspace_id) = self.workspace_for_path(&path) {
            self.focus_workspace(workspace_id, cx);
        } else if self.active.is_none() {
            // No current workspace: open the file's parent as a workspace root.
            if let Some(parent) = path.parent() {
                self.register_workspace(parent.to_path_buf(), cx);
            }
        }
        // Else: keep current workspace even when the file is outside it.
        self.open_editor(path, true, cx);
    }

    /// Longest registry root (open or closed) that is a prefix of `path`.
    fn workspace_for_path(&self, path: &Path) -> Option<WorkspaceId> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut best: Option<(usize, WorkspaceId)> = None;
        for rec in self
            .registry
            .workspaces
            .iter()
            .chain(self.registry.closed_workspaces.iter())
        {
            let root = rec.root.canonicalize().unwrap_or_else(|_| rec.root.clone());
            if path.starts_with(&root) {
                let len = root.as_os_str().len();
                if best.is_none_or(|(best_len, _)| len > best_len) {
                    best = Some((len, rec.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    fn focus_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if self.registry.workspace(id).is_some() {
            self.activate_workspace(id, cx);
            return;
        }
        if self.registry.closed_workspaces.iter().any(|w| w.id == id) {
            self.reopen_workspace(id, cx);
        }
    }

    /// Open Workspace palette from keyboard (⌘⇧O). Includes other open
    /// workspaces (not current) for jump, plus closed archive for reopen.
    /// Second invoke while open → Browse.
    pub(super) fn add_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace_picker.is_some() {
            self.workspace_picker = None;
            self.browse_for_workspace(cx);
            return;
        }
        self.open_workspace_picker(WorkspacePickerMode::Keyboard, window, cx);
    }

    /// Open Workspace palette from the sidebar + (reopen/discover only — no open list).
    pub(crate) fn add_workspace_from_plus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace_picker.is_some() {
            self.workspace_picker = None;
            self.browse_for_workspace(cx);
            return;
        }
        self.open_workspace_picker(WorkspacePickerMode::Plus, window, cx);
    }

    fn open_workspace_picker(
        &mut self,
        mode: WorkspacePickerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finder = None;
        self.task_picker = None;
        self.command_palette = None;
        self.deferred.restore_pane = self.focused_pane(window, cx);
        let current = self.active;
        let mut known = Vec::new();
        for rec in &self.registry.closed_workspaces {
            let missing = !crate::workspace_discover::path_is_dir(&rec.root);
            known.push(WorkspaceCandidate::Closed {
                id: rec.id,
                name: rec.name.clone(),
                root: rec.root.clone(),
                missing,
                last_opened: rec.last_opened.unwrap_or(0),
            });
        }
        // Keyboard: jump among open workspaces, excluding the current one.
        // Plus: only closed/discover — open ones are already in the sidebar.
        if mode == WorkspacePickerMode::Keyboard {
            for rec in &self.registry.workspaces {
                if current == Some(rec.id) {
                    continue;
                }
                known.push(WorkspaceCandidate::Open {
                    id: rec.id,
                    name: rec.name.clone(),
                    root: rec.root.clone(),
                    last_opened: rec.last_opened.unwrap_or(0),
                });
            }
        }
        let picker = cx.new(|cx| WorkspacePickerView::new(known, cx));
        self._workspace_picker_sub = Some(cx.subscribe(&picker, Self::on_workspace_picker_event));
        self.workspace_picker = Some(picker);
        cx.notify();
    }

    pub(crate) fn toggle_workspaces_section(&mut self, cx: &mut Context<Self>) {
        self.workspaces_collapsed = !self.workspaces_collapsed;
        persist_section_prefs(self.workspaces_collapsed, self.file_browser.is_open());
        cx.notify();
    }

    pub(crate) fn workspaces_collapsed(&self) -> bool {
        self.workspaces_collapsed
    }

    fn on_workspace_picker_event(
        &mut self,
        _picker: Entity<WorkspacePickerView>,
        event: &WorkspacePickerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspacePickerEvent::Open(candidate) => {
                self.workspace_picker = None;
                self.deferred.restore_pane = None;
                self.apply_workspace_pick(candidate, cx);
            }
            WorkspacePickerEvent::Forget(id) => {
                self.forget_closed_workspace(*id);
            }
            WorkspacePickerEvent::Browse => {
                self.workspace_picker = None;
                self.browse_for_workspace(cx);
            }
            WorkspacePickerEvent::Dismissed => {
                self.workspace_picker = None;
                self.deferred.pending_focus = self
                    .deferred
                    .restore_pane
                    .take()
                    .or_else(|| Some(self.fallback_content_pane()));
                cx.notify();
            }
        }
    }

    /// Drop a closed workspace from archive (palette ⌘⌫). Deletes session file.
    fn forget_closed_workspace(&mut self, id: WorkspaceId) {
        let before = self.registry.closed_workspaces.len();
        self.registry.closed_workspaces.retain(|w| w.id != id);
        if self.registry.closed_workspaces.len() == before {
            return;
        }
        if let Err(error) = xenon_store::delete_session(id) {
            log::error!("forget_closed_workspace: delete session {id}: {error}");
        }
        save_registry(&self.registry, "forget_closed_workspace");
    }

    fn apply_workspace_pick(&mut self, candidate: &WorkspaceCandidate, cx: &mut Context<Self>) {
        match candidate {
            WorkspaceCandidate::Open { id, root, .. } => {
                if !crate::workspace_discover::path_is_dir(root) {
                    return;
                }
                if self.registry.workspace(*id).is_some() {
                    self.activate_workspace(*id, cx);
                }
            }
            WorkspaceCandidate::Closed {
                id, missing: true, ..
            } => {
                let _ = id; // never reopen missing
            }
            WorkspaceCandidate::Closed { id, root, .. } => {
                if !crate::workspace_discover::path_is_dir(root) {
                    return;
                }
                self.reopen_workspace(*id, cx);
            }
            WorkspaceCandidate::Path { root, .. } => {
                if !crate::workspace_discover::path_is_dir(root) {
                    return;
                }
                self.register_workspace(root.clone(), cx);
            }
        }
    }

    /// Native macOS folder picker (Browse… escape hatch).
    pub(super) fn browse_for_workspace(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.register_workspace(path, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn register_workspace(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        let Some(root) = crate::workspace_discover::resolve_existing_dir(&root) else {
            return;
        };
        if let Some(record) = self
            .registry
            .workspaces
            .iter()
            .find(|record| same_workspace_root(&record.root, &root))
        {
            self.activate_workspace(record.id, cx);
            return;
        }
        if let Some(record) = self
            .registry
            .closed_workspaces
            .iter()
            .find(|record| same_workspace_root(&record.root, &root))
        {
            self.reopen_workspace(record.id, cx);
            return;
        }
        // Drop stale closed entries with same basename but missing path.
        let basename = root.file_name().map(|n| n.to_os_string());
        if let Some(base) = basename {
            self.registry.closed_workspaces.retain(|rec| {
                if crate::workspace_discover::path_is_dir(&rec.root) {
                    return true;
                }
                rec.root.file_name().is_none_or(|n| n != base.as_os_str())
            });
        }
        let record = WorkspaceRec::new(root);
        let workspace_id = record.id;
        let session = SessionState {
            sidebar_visible: !self.sidebar_collapsed,
            sidebar_width: xenon_core::clamp_sidebar(self.sidebar_width),
            content: Default::default(),
        };
        self.registry.workspaces.push(record);
        self.sessions.insert(workspace_id, session.clone());
        save_session(workspace_id, &session, "register_workspace");
        save_registry(&self.registry, "register_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        self.activate_workspace(workspace_id, cx);
    }

    /// Move workspace `dragged` to `target`'s position in the sidebar.
    pub(crate) fn reorder_workspace(
        &mut self,
        dragged: WorkspaceId,
        target: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        if dragged == target {
            return;
        }
        let list = &mut self.registry.workspaces;
        let Some(from) = list.iter().position(|w| w.id == dragged) else {
            return;
        };
        let record = list.remove(from);
        let to = list
            .iter()
            .position(|w| w.id == target)
            .unwrap_or(list.len());
        list.insert(to, record);
        save_registry(&self.registry, "reorder_workspace");
        cx.notify();
    }

    /// Move workspace `dragged` to the end of the open list (bottom drop zone).
    pub(crate) fn reorder_workspace_to_end(
        &mut self,
        dragged: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        let list = &mut self.registry.workspaces;
        let Some(from) = list.iter().position(|w| w.id == dragged) else {
            return;
        };
        if from + 1 == list.len() {
            return;
        }
        let record = list.remove(from);
        list.push(record);
        save_registry(&self.registry, "reorder_workspace");
        cx.notify();
    }

    /// Drop workspace live views with no dirty check. `window` restores focus when present.
    pub(super) fn force_close_workspace(
        &mut self,
        id: WorkspaceId,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .registry
            .workspaces
            .iter()
            .position(|workspace| workspace.id == id)
        else {
            return;
        };
        let record = self.registry.workspaces.remove(index);
        self.lsp.stop_workspace(&record.root);
        let closed_active = self.active == Some(id);
        let dead_content = self.contents.remove(&id);
        self.sessions.remove(&id);
        self.attention.remove(&id);
        self.recent_files.remove(&id);
        // Drop the finder index for this root so it can be rebuilt if reopened.
        self.file_indexes.remove(&record.root);
        self.index_tasks.remove(&record.root);
        self.services.git_dirt.remove(&id);
        self.registry.closed_workspaces.push(record);
        if closed_active {
            self.active = None;
            self.registry.active = None;
        }
        save_registry(&self.registry, "close_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        if closed_active {
            if let Some(next) = self.first_workspace() {
                match window {
                    Some(window) => self.select_workspace(next, window, cx),
                    None => {
                        self.activate_workspace(next, cx);
                        self.focus_after_teardown(None, cx);
                    }
                }
            } else {
                self.focus_after_teardown(window, cx);
            }
        }
        cx.notify();
        // Yield once so the sidebar/main panel repaint, then drop PTYs/editors.
        cx.spawn(async move |_app, cx| {
            cx.background_executor()
                .timer(std::time::Duration::ZERO)
                .await;
            drop(dead_content);
        })
        .detach();
    }

    pub(crate) fn reopen_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(index) = self
            .registry
            .closed_workspaces
            .iter()
            .position(|workspace| workspace.id == id)
        else {
            return;
        };
        // Never reopen a root that no longer exists on disk.
        if !crate::workspace_discover::path_is_dir(&self.registry.closed_workspaces[index].root) {
            return;
        }
        let record = self.registry.closed_workspaces.remove(index);
        let session = xenon_store::load_session(record.id).unwrap_or_else(|_| SessionState {
            sidebar_visible: !self.sidebar_collapsed,
            sidebar_width: xenon_core::clamp_sidebar(self.sidebar_width),
            content: Default::default(),
        });
        self.sessions.insert(record.id, session);
        self.registry.workspaces.push(record);
        save_registry(&self.registry, "reopen_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        self.activate_workspace(id, cx);
    }
}

/// How the Open Workspace palette was launched.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspacePickerMode {
    /// ⌘⇧O: closed + other open (not current).
    Keyboard,
    /// Sidebar +: closed + discover only.
    Plus,
}

fn same_workspace_root(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => a == b,
    }
}
