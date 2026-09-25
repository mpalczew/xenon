//! File-navigation support for the finder: the background workspace index that
//! powers cmd-p and cmd-click resolution, plus capturing/restoring which pane
//! held keyboard focus around the finder.

use super::*;
use xenon_core::PaneId;
use xenon_settings::{Copy, CopyClean, CopyCode, Cut, Paste};

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

    /// Resolve a live Xenon focus owner. GPUI may have no focused element while
    /// the window is inactive, but app commands always have a logical fallback.
    pub(super) fn current_focus_owner(&self, window: &Window, cx: &Context<Self>) -> FocusOwner {
        if let Some(pane) = self.leaf_with_gpui_focus(window, cx)
            && let Some(tab) = self
                .active_content()
                .and_then(|content| content.root.as_ref())
                .and_then(|root| root.find_leaf(pane))
                .and_then(|leaf| leaf.active_tab())
        {
            return match tab {
                LiveTab::Terminal { .. } => FocusOwner::Terminal,
                LiveTab::Editor { .. } => FocusOwner::Editor,
            };
        }
        if self.browser_focused {
            return FocusOwner::Browser;
        }
        match self.active_content().and_then(|c| c.active_tab()) {
            Some(LiveTab::Terminal { .. }) => FocusOwner::Terminal,
            Some(LiveTab::Editor { .. }) => FocusOwner::Editor,
            None => {
                if self.active_terminal().is_some() {
                    FocusOwner::Terminal
                } else if self.active_editor().is_some() {
                    FocusOwner::Editor
                } else {
                    FocusOwner::Shell
                }
            }
        }
    }

    pub(super) fn focus_owner(
        &mut self,
        pane: FocusOwner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pane {
            FocusOwner::Terminal => self.focus_terminal(window, cx),
            FocusOwner::Editor => self.focus_editor(window, cx),
            FocusOwner::Browser => self.focus_browser(window, cx),
            FocusOwner::Shell => {
                self.browser_focused = false;
                self.focus.focus(window, cx);
                cx.notify();
            }
        }
    }

    pub(super) fn fallback_content_pane(&self) -> FocusOwner {
        match self.deferred.last_font_pane {
            FontPane::Editor if self.has_editor() => FocusOwner::Editor,
            FontPane::Terminal if self.active_content().is_some_and(|c| c.has_terminal()) => {
                FocusOwner::Terminal
            }
            _ if self.active_content().is_some_and(|c| c.has_terminal()) => FocusOwner::Terminal,
            _ if self.has_editor() => FocusOwner::Editor,
            _ => FocusOwner::Shell,
        }
    }

    /// Teardown always transfers focus to a live surface or the shell.
    pub(crate) fn focus_after_teardown(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let target = self.fallback_content_pane();
        if let Some(window) = window {
            log::info!(
                "focus transfer: target={target:?}; window_active={}; gpui_focus_before={:?}",
                window.is_window_active(),
                window.focused(cx),
            );
            self.focus_owner(target, window, cx);
            log::info!(
                "focus transfer complete: target={target:?}; window_active={}; gpui_focus_after={:?}",
                window.is_window_active(),
                window.focused(cx),
            );
        } else {
            log::info!("focus transfer deferred: target={target:?}; window unavailable");
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
        let target = match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => FontPane::Editor,
            FocusOwner::Terminal => FontPane::Terminal,
            FocusOwner::Browser | FocusOwner::Shell => self.deferred.last_font_pane,
        };
        self.deferred.last_font_pane = target;
        target
    }

    pub(super) fn bind_path_actions(&self, root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        root.on_action(cx.listener(|this, _: &crate::CloseOtherTabs, window, cx| {
            this.close_other_tabs_focused(window, cx);
        }))
        .on_action(cx.listener(|this, _: &crate::CopyPath, _, cx| {
            this.copy_focused_path(false, cx);
        }))
        .on_action(cx.listener(|this, _: &crate::CopyRelativePath, _, cx| {
            this.copy_focused_path(true, cx);
        }))
        .on_action(cx.listener(|this, _: &crate::RevealInFinder, _, _cx| {
            this.reveal_focused_path();
        }))
        .on_action(cx.listener(|this, _: &crate::OpenInDefaultApp, _, cx| {
            this.open_focused_in_default_app(cx);
        }))
    }

    pub(super) fn bind_clipboard_actions(
        &self,
        root: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        root.on_action(cx.listener(|this, _: &Cut, window, cx| {
            if !this.text_field_open() {
                this.clipboard_cut(window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &Copy, window, cx| {
            if !this.text_field_open() {
                this.clipboard_copy(window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &CopyClean, window, cx| {
            if !this.text_field_open() {
                this.clipboard_copy_clean(window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &CopyCode, window, cx| {
            if !this.text_field_open() {
                this.clipboard_copy_code(window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &Paste, window, cx| {
            if !this.text_field_open() {
                this.clipboard_paste(window, cx);
            }
        }))
    }

    /// Palettes and the inline rename field own clipboard, not the session tab.
    pub(crate) fn text_field_open(&self) -> bool {
        self.finder.is_some()
            || self.task_picker.is_some()
            || self.workspace_picker.is_some()
            || self.workspace_create.is_some()
            || self.command_palette.is_some()
            || self.theme_picker.is_some()
            || self.worklist_capture_visible.is_some()
            || self.renaming.is_some()
    }

    pub(super) fn clipboard_cut(&self, window: &Window, cx: &mut Context<Self>) {
        match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.cut_selection(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Terminal => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.cut_selection(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Browser | FocusOwner::Shell => {}
        }
    }

    pub(super) fn clipboard_copy(&self, window: &Window, cx: &mut Context<Self>) {
        match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.copy_selection(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Terminal => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.copy_selection(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Browser | FocusOwner::Shell => {}
        }
    }

    pub(super) fn clipboard_copy_clean(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => self.clipboard_copy(window, cx),
            FocusOwner::Terminal => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.copy_clean_selection(window, cx);
                        cx.notify();
                    });
                } else {
                    window.play_system_bell();
                }
            }
            FocusOwner::Browser | FocusOwner::Shell => window.play_system_bell(),
        }
    }

    pub(super) fn clipboard_copy_code(&self, window: &Window, cx: &mut Context<Self>) {
        match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => self.clipboard_copy(window, cx),
            FocusOwner::Terminal => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.copy_code_selection(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Browser | FocusOwner::Shell => {}
        }
    }

    pub(super) fn clipboard_paste(&self, window: &Window, cx: &mut Context<Self>) {
        match self.current_focus_owner(window, cx) {
            FocusOwner::Editor => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.paste_clipboard(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Terminal => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.paste_clipboard(cx);
                        cx.notify();
                    });
                }
            }
            FocusOwner::Browser | FocusOwner::Shell => {}
        }
    }
}
