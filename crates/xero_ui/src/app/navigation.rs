//! File-navigation support for the finder: the background workspace index that
//! powers cmd-p and cmd-click resolution, plus capturing/restoring which pane
//! held keyboard focus around the finder.

use super::*;

impl XeroApp {
    /// Walk `root` into `file_indexes` on a background thread. Skips the walk when
    /// an index already exists unless `force` (explicit refresh). Prefer
    /// `force: false` on cmd-p open so large roots reuse the cache. Emits
    /// partial snapshots so cmd-p can search before the walk finishes.
    /// Idempotent per root: a second call replaces (cancels) the previous build.
    pub(super) fn reindex(&mut self, root: PathBuf, force: bool, cx: &mut Context<Self>) {
        if !force && self.file_indexes.contains_key(&root) {
            return;
        }
        let key = root.clone();
        let build_root = root.clone();
        // Bounded(1): walk drops intermediate snapshots if the UI is behind.
        let (tx, rx) = async_channel::bounded::<Arc<FileIndex>>(1);
        let walk = cx.background_executor().spawn(async move {
            FileIndex::build_with_progress(&build_root, |partial| {
                let snap = Arc::new(partial.clone());
                // Coalesce: keep at most one pending snapshot for the UI.
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
            // Ensure walk finishes (and any last force_send is drained above).
            let final_index = walk.await;
            app.update(cx, |app, cx| {
                app.install_index(root, Arc::new(final_index), true, cx);
            })
            .ok();
        });
        self.index_tasks.insert(key, task);
    }

    /// Store an index snapshot. `done` clears the in-flight task for this root.
    /// If the finder is open on this root, hand it the index immediately.
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
        // Prefer a larger partial over a smaller one if races reorder.
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

    /// Which pane currently holds keyboard focus (for finder focus restore).
    pub(super) fn focused_pane(&self, window: &Window, cx: &Context<Self>) -> Option<FocusPane> {
        if self.browser_focused {
            return Some(FocusPane::Browser);
        }
        if self
            .active_terminal()
            .is_some_and(|t| t.read(cx).focus_handle(cx).contains_focused(window, cx))
        {
            Some(FocusPane::Terminal)
        } else if self
            .active_editor()
            .is_some_and(|e| e.read(cx).focus_handle(cx).contains_focused(window, cx))
        {
            Some(FocusPane::Editor)
        } else {
            None
        }
    }

    /// Return keyboard focus to `pane` (the terminal or editor of the active workspace).
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
        }
    }

    /// Last content pane that can take keyboard focus (never None).
    /// Used when an overlay dies without a remembered restore target.
    pub(super) fn fallback_content_pane(&self) -> FocusPane {
        match self.deferred.last_font_pane {
            FontPane::Editor if self.has_editor() => FocusPane::Editor,
            FontPane::Terminal if self.terminal_visible() => FocusPane::Terminal,
            _ if self.terminal_visible() => FocusPane::Terminal,
            _ if self.has_editor() => FocusPane::Editor,
            _ => FocusPane::Terminal,
        }
    }

    /// cmd-+ / cmd--: nudge only the focused editor or terminal font size.
    /// Sidebar / tree / no focus falls back to the last content pane.
    pub(super) fn nudge_font_size(
        &mut self,
        delta: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.font_target(window, cx) {
            FontPane::Editor => xero_settings::nudge_editor_font_size(cx, delta),
            FontPane::Terminal => xero_settings::nudge_terminal_font_size(cx, delta),
        }
        xero_settings::save(cx);
        window.refresh();
    }

    /// cmd-0: reset font size for the focused editor or terminal only.
    pub(super) fn reset_font_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.font_target(window, cx) {
            FontPane::Editor => xero_settings::reset_editor_font_size(cx),
            FontPane::Terminal => xero_settings::reset_terminal_font_size(cx),
        }
        xero_settings::save(cx);
        window.refresh();
    }

    /// Which content surface zoom / reset should affect.
    fn font_target(&mut self, window: &Window, cx: &Context<Self>) -> FontPane {
        let target = match self.focused_pane(window, cx) {
            Some(FocusPane::Editor) => FontPane::Editor,
            Some(FocusPane::Terminal) => FontPane::Terminal,
            Some(FocusPane::Browser) | None => self.deferred.last_font_pane,
        };
        self.deferred.last_font_pane = target;
        target
    }

    /// Route Cut to the focused editor or terminal (app menu / global binding).
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
            Some(FocusPane::Terminal) | Some(FocusPane::Browser) | None => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.cut_selection(cx);
                        cx.notify();
                    });
                }
            }
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
            Some(FocusPane::Terminal) | Some(FocusPane::Browser) | None => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.copy_selection(cx);
                        cx.notify();
                    });
                }
            }
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
            Some(FocusPane::Terminal) | Some(FocusPane::Browser) | None => {
                if let Some(terminal) = self.active_terminal() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.paste_clipboard(cx);
                        cx.notify();
                    });
                }
            }
        }
    }
}
