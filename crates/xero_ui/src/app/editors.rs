use super::*;

impl XeroApp {
    pub(super) fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.open_editor(path, true, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, focus: bool, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            log::warn!("open_editor: no active stream for {}", path.display());
            return;
        };
        self.editor_collapsed = false;
        self.save_layout(id);
        let stack = self.editors.entry(id).or_default();
        // Focus an already-open tab rather than opening a duplicate.
        if let Some(index) = stack.tabs.iter().position(|tab| tab.path == path) {
            stack.active = index;
        } else {
            match EditorView::build(path.clone(), focus, cx) {
                Ok(view) => {
                    let name = file_name(&path);
                    let stack = self.editors.entry(id).or_default();
                    stack.tabs.push(EditorTab { path, name, view });
                    stack.active = stack.tabs.len() - 1;
                }
                Err(error) => log::error!("open failed: {error}"),
            }
        }
        self.finder = None;
        if self.file_browser.is_open() {
            self.reveal_active_file(cx);
        }
        cx.notify();
    }

    /// The open tabs and focused index for the active stream.
    pub(crate) fn editor_stack(&self) -> Option<&EditorStack> {
        self.active.and_then(|id| self.editors.get(&id))
    }

    pub(crate) fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.active
            && let Some(stack) = self.editors.get_mut(&id)
            && index < stack.tabs.len()
        {
            stack.active = index;
            let view = stack.tabs[index].view.clone();
            view.update(cx, |view, cx| view.sync_from_disk(cx));
            cx.notify();
        }
    }

    /// Re-check open text editors whose paths match any of `paths` (FS events).
    pub(crate) fn sync_editors_for_paths(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        for stack in self.editors.values() {
            for tab in &stack.tabs {
                let open = tab.view.read(cx).path().to_path_buf();
                if paths.iter().any(|p| p == &open || open.starts_with(p)) {
                    tab.view.update(cx, |view, cx| view.sync_from_disk(cx));
                }
            }
        }
    }

    /// Close the tab at `index` in the active stream; drops the stack when empty.
    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.active
            && let Some(stack) = self.editors.get_mut(&id)
            && index < stack.tabs.len()
        {
            stack.tabs.remove(index);
            if stack.tabs.is_empty() {
                self.editors.remove(&id);
                self.editor_collapsed = true;
                self.save_layout(id);
            } else {
                fix_editor_active_after_remove(&mut stack.active, index, stack.tabs.len());
            }
            cx.notify();
        }
    }

    /// Move an editor tab to another stream in the same workspace. If the dest
    /// already has that path open, focus the existing tab and drop the moved one.
    pub(crate) fn move_editor_tab(
        &mut self,
        tab: TabMove,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab.from == tab.to || !self.same_workspace(tab.from, tab.to) {
            return;
        }
        let Some(editor_tab) = self.take_editor_tab(tab.from, tab.index) else {
            return;
        };
        let stack = self.editors.entry(tab.to).or_default();
        if let Some(existing) = stack.tabs.iter().position(|t| t.path == editor_tab.path) {
            stack.active = existing;
            // `editor_tab` drops: duplicate path at dest keeps the open view.
        } else {
            stack.tabs.push(editor_tab);
            stack.active = stack.tabs.len() - 1;
        }
        self.editor_collapsed = false;
        self.save_layout(tab.to);
        self.activate_stream(tab.to, cx);
        if let Some(editor) = self.active_editor() {
            editor.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn move_editor_tab_to_new_stream(
        &mut self,
        from: StreamId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.workspace_of(from).map(|w| w.id) else {
            return;
        };
        let Some(to) = self.create_stream(workspace) else {
            return;
        };
        self.move_editor_tab(TabMove { from, index, to }, window, cx);
    }

    fn take_editor_tab(&mut self, from: StreamId, index: usize) -> Option<EditorTab> {
        let (tab, emptied) = {
            let stack = self.editors.get_mut(&from)?;
            if index >= stack.tabs.len() {
                return None;
            }
            let tab = stack.tabs.remove(index);
            let emptied = stack.tabs.is_empty();
            if !emptied {
                fix_editor_active_after_remove(&mut stack.active, index, stack.tabs.len());
            }
            (tab, emptied)
        };
        if emptied {
            self.editors.remove(&from);
            if self.active == Some(from) {
                self.editor_collapsed = true;
                self.save_layout(from);
            }
        }
        Some(tab)
    }

    pub(crate) fn save_active_editor(&self, cx: &mut Context<Self>) {
        if let Some(editor) = self.active_editor() {
            editor.update(cx, |editor, cx| editor.save(cx));
        }
    }

    /// Close the focused tab (Cmd-W / toolbar).
    pub(crate) fn close_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(stack) = self.editor_stack() {
            let active = stack.active;
            self.close_tab(active, cx);
        }
    }

    pub(crate) fn has_editor(&self) -> bool {
        self.editor_stack()
            .is_some_and(|stack| !stack.tabs.is_empty())
    }

    pub(crate) fn active_editor(&self) -> Option<Entity<EditorView>> {
        self.editor_stack()
            .and_then(|s| s.tabs.get(s.active))
            .map(|tab| tab.view.clone())
    }

    /// Whether the focused editor is a markdown file (drives the Preview button).
    pub(crate) fn active_editor_is_markdown(&self, cx: &App) -> bool {
        self.active_editor()
            .is_some_and(|view| view.read(cx).is_markdown())
    }

    pub(crate) fn active_editor_is_previewing(&self, cx: &App) -> bool {
        self.active_editor()
            .is_some_and(|view| view.read(cx).is_previewing())
    }

    /// Toggle the focused markdown editor between source and preview.
    pub(crate) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.active_editor() {
            view.update(cx, |view, cx| view.toggle_preview(cx));
        }
    }
}

fn fix_editor_active_after_remove(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}
