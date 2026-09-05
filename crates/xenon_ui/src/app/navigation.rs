//! File-navigation support for the finder: the background workspace index that
//! powers cmd-p and cmd-click resolution, plus capturing/restoring which pane
//! held keyboard focus around the finder.

use super::*;
use xenon_core::PaneId;

/// Cap on per-workspace recently opened paths used for cmd-p ranking.
const MAX_RECENT_FILES: usize = 64;

impl XenonApp {
    /// Record `path` as most-recently opened for its workspace (relative paths).
    pub(super) fn touch_recent_file(&mut self, workspace: WorkspaceId, path: &Path) {
        let Some(root) = self.workspace_root(workspace) else {
            return;
        };
        let relative = path
            .strip_prefix(&root)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| path.to_path_buf());
        if relative.as_os_str().is_empty() {
            return;
        }
        let list = self.recent_files.entry(workspace).or_default();
        list.retain(|p| p != &relative);
        list.insert(0, relative);
        list.truncate(MAX_RECENT_FILES);
    }

    /// Snapshot of MRU relative paths for ranking (most-recent first).
    pub(super) fn recent_files_for(&self, workspace: WorkspaceId) -> Vec<PathBuf> {
        self.recent_files
            .get(&workspace)
            .cloned()
            .unwrap_or_default()
    }

    /// Build or refresh the file index for `root` on a background thread.
    ///
    /// - `force == false`: skip when a finished cache already exists (used for
    ///   session switch / cmd-click warm path).
    /// - `force == true`: re-walk even when cached so new files appear. Does not
    ///   clear the cache first; the open finder keeps serving it until partials
    ///   / final land via [`install_index`].
    /// - Either way: if a walk is already in flight for this root, leave it
    ///   alone (it already streams into the cache/finder).
    pub(super) fn reindex(&mut self, root: PathBuf, force: bool, cx: &mut Context<Self>) {
        if self.index_tasks.contains_key(&root) {
            return;
        }
        if !force && self.file_indexes.contains_key(&root) {
            return;
        }
        let key = root.clone();
        let build_root = root.clone();
        let (tx, rx) = async_channel::bounded::<Arc<FileIndex>>(1);
        let walk = cx.background_executor().spawn(async move {
            FileIndex::build_with_progress(&build_root, |partial| {
                let snap = Arc::new(partial.clone());
                match tx.try_send(snap) {
                    Ok(()) => true,
                    Err(async_channel::TrySendError::Closed(_)) => false,
                    Err(async_channel::TrySendError::Full(snap)) => {
                        let _ = tx.force_send(snap);
                        true
                    }
                }
            })
        });
        let task = cx.spawn(async move |app, cx| {
            while let Ok(partial) = rx.recv().await {
                let ok = app
                    .update(cx, |app, cx| {
                        app.install_index(root.clone(), partial, false, cx)
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
            let final_index = walk.await;
            app.update(cx, |app, cx| {
                app.install_index(root, Arc::new(final_index), true, cx);
            })
            .ok();
        });
        self.index_tasks.insert(key, task);
    }

    fn install_index(
        &mut self,
        root: PathBuf,
        index: Arc<FileIndex>,
        done: bool,
        cx: &mut Context<Self>,
    ) {
        if done {
            self.index_tasks.remove(&root);
        }
        if let Some(existing) = self.file_indexes.get(&root)
            && existing.len() > index.len()
            && !done
        {
            return;
        }
        if self.active.and_then(|id| self.workspace_root(id)).as_ref() == Some(&root)
            && let Some(finder) = &self.finder
        {
            finder.update(cx, |finder, cx| finder.set_index(index.clone(), cx));
        }
        self.file_indexes.insert(root, index);
    }

    pub(super) fn focused_pane(&self, window: &Window, cx: &Context<Self>) -> Option<FocusPane> {
        if let Some(pane) = self.leaf_with_gpui_focus(window, cx) {
            return match self
                .active_content()?
                .root
                .as_ref()?
                .find_leaf(pane)?
                .active_tab()?
            {
                LiveTab::Terminal { .. } => Some(FocusPane::Terminal),
                LiveTab::Editor { .. } => Some(FocusPane::Editor),
            };
        }
        if self.browser_focused {
            return Some(FocusPane::Browser);
        }
        match self.active_content().and_then(|c| c.active_tab()) {
            Some(LiveTab::Terminal { .. }) => Some(FocusPane::Terminal),
            Some(LiveTab::Editor { .. }) => Some(FocusPane::Editor),
            None => {
                if self.active_terminal().is_some() {
                    Some(FocusPane::Terminal)
                } else if self.active_editor().is_some() {
                    Some(FocusPane::Editor)
                } else {
                    None
                }
            }
        }
    }

    pub(super) fn focus_pane(
        &mut self,
        pane: FocusPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pane {
            FocusPane::Terminal => self.focus_terminal(window, cx),
            FocusPane::Editor => self.focus_editor(window, cx),
            FocusPane::Browser => self.focus_browser(window, cx),
            FocusPane::Shell => {
                self.browser_focused = false;
                self.focus.focus(window, cx);
                cx.notify();
            }
        }
    }

    pub(super) fn fallback_content_pane(&self) -> FocusPane {
        match self.deferred.last_font_pane {
            FontPane::Editor if self.has_editor() => FocusPane::Editor,
            FontPane::Terminal if self.active_content().is_some_and(|c| c.has_terminal()) => {
                FocusPane::Terminal
            }
            _ if self.active_content().is_some_and(|c| c.has_terminal()) => FocusPane::Terminal,
            _ if self.has_editor() => FocusPane::Editor,
            _ => FocusPane::Shell,
        }
    }

    /// Teardown always transfers focus to a live surface or the shell.
    pub(super) fn focus_after_teardown(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let target = self.fallback_content_pane();
        if let Some(window) = window {
            self.focus_pane(target, window, cx);
        } else {
            self.deferred.pending_focus = Some(target);
            cx.notify();
        }
    }

    /// After a workspace becomes active, put keys on its remembered leaf.
    /// Existing terminals/editors skip `focused_once`, so a switch that does
    /// not transfer focus leaves the shell `track_focus` handle owning keys.
    pub(super) fn focus_workspace_leaf(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let pane = self.active_content().and_then(|c| {
            let id = c.focused?;
            c.root.as_ref()?.find_leaf(id)?;
            Some(id)
        });
        if let Some(pane) = pane {
            self.focus_leaf_now_or_later(pane, window, cx);
        } else {
            self.focus_after_teardown(window, cx);
        }
    }

    /// Focus a leaf now, or on the next paint when the caller has no `Window`
    /// (PTY auto-close, dirty-close, vim `:q`).
    pub(super) fn focus_leaf_now_or_later(
        &mut self,
        pane: PaneId,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(window) = window {
            self.focus_leaf_active(pane, window, cx);
        } else {
            self.deferred.pending_leaf = Some(pane);
            cx.notify();
        }
    }

    pub(super) fn nudge_font_size(
        &mut self,
        delta: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.font_target(window, cx) {
            FontPane::Editor => xenon_settings::nudge_editor_font_size(cx, delta),
            FontPane::Terminal => xenon_settings::nudge_terminal_font_size(cx, delta),
        }
        xenon_settings::save(cx);
        window.refresh();
    }

    pub(super) fn reset_font_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.font_target(window, cx) {
            FontPane::Editor => xenon_settings::reset_editor_font_size(cx),
            FontPane::Terminal => xenon_settings::reset_terminal_font_size(cx),
        }
        xenon_settings::save(cx);
        window.refresh();
    }

    fn font_target(&mut self, window: &Window, cx: &Context<Self>) -> FontPane {
        let target = match self.focused_pane(window, cx) {
            Some(FocusPane::Editor) => FontPane::Editor,
            Some(FocusPane::Terminal) => FontPane::Terminal,
            Some(FocusPane::Browser | FocusPane::Shell) | None => self.deferred.last_font_pane,
        };
        self.deferred.last_font_pane = target;
        target
    }

    pub(super) fn clipboard_cut(&self, window: &Window, cx: &mut Context<Self>) {
        match self.focused_pane(window, cx) {
            Some(FocusPane::Editor) => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.cut_selection(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Terminal) => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.cut_selection(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Browser | FocusPane::Shell) | None => {}
        }
    }

    pub(super) fn clipboard_copy(&self, window: &Window, cx: &mut Context<Self>) {
        match self.focused_pane(window, cx) {
            Some(FocusPane::Editor) => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.copy_selection(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Terminal) => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.copy_selection(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Browser | FocusPane::Shell) | None => {}
        }
    }

    pub(super) fn clipboard_paste(&self, window: &Window, cx: &mut Context<Self>) {
        match self.focused_pane(window, cx) {
            Some(FocusPane::Editor) => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.paste_clipboard(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Terminal) => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.paste_clipboard(cx);
                        cx.notify();
                    });
                }
            }
            Some(FocusPane::Browser | FocusPane::Shell) | None => {}
        }
    }
}
