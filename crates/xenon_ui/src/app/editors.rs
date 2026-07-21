use super::*;

impl XenonApp {
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

    /// Create a new empty file (path prompt under workspace root).
    pub(super) fn new_file_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(root) = self.workspace_root(id) else {
            return;
        };
        let rx = cx.prompt_for_new_path(&root, Some("untitled.txt"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |this, cx| {
                    if let Some(parent) = path.parent()
                        && !parent.as_os_str().is_empty()
                        && let Err(error) = std::fs::create_dir_all(parent)
                    {
                        log::error!("new file mkdir failed: {error}");
                        return;
                    }
                    if !path.exists()
                        && let Err(error) = std::fs::write(&path, b"")
                    {
                        log::error!("new file create failed: {error}");
                        return;
                    }
                    this.open_editor(path, true, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Save As for the active editor.
    pub(super) fn save_as_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor() else {
            return;
        };
        let current = editor.read(cx).path().to_path_buf();
        let dir = current
            .parent()
            .map(|p| p.to_path_buf())
            .or_else(|| self.active.and_then(|id| self.workspace_root(id)))
            .unwrap_or_else(|| PathBuf::from("."));
        let name = current
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled.txt");
        let rx = cx.prompt_for_new_path(&dir, Some(name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |this, cx| {
                    if let Some(ed) = this.active_editor() {
                        ed.update(cx, |ed, cx| ed.save_as(path, cx));
                    }
                })
                .ok();
            }
        })
        .detach();
    }
}
