//! File-navigation support for the finder: the background workspace index that
//! powers cmd-p and cmd-click resolution, plus capturing/restoring which pane
//! held keyboard focus around the finder.

use super::*;

impl XeroApp {
    /// Walk `root` into `file_indexes` on a background thread. Skips the walk when
    /// an index already exists unless `force` (a refresh). Idempotent per root:
    /// a second call for a root already building replaces the in-flight build.
    pub(super) fn reindex(&mut self, root: PathBuf, force: bool, cx: &mut Context<Self>) {
        if !force && self.file_indexes.contains_key(&root) {
            return;
        }
        let key = root.clone();
        let build_root = root.clone();
        let task = cx.spawn(async move |app, cx| {
            let index = cx
                .background_executor()
                .spawn(async move { FileIndex::build(&build_root) })
                .await;
            app.update(cx, |app, cx| app.install_index(root, Arc::new(index), cx))
                .ok();
        });
        self.index_tasks.insert(key, task);
    }

    /// Store a freshly built index and, if the finder is open on this root, hand
    /// it the index so results appear without the user retyping.
    fn install_index(&mut self, root: PathBuf, index: Arc<FileIndex>, cx: &mut Context<Self>) {
        self.index_tasks.remove(&root);
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
            Some(FocusPane::Browser) | None => self.last_font_pane,
        };
        self.last_font_pane = target;
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
