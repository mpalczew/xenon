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
        self.new_file_in_dir(root, cx);
    }

    /// Create a new empty file with the save dialog starting in `dir`.
    pub(crate) fn new_file_in_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let mut path = dir.join("untitled");
        let mut index = 2;
        while path.exists() {
            path = dir.join(format!("untitled-{index}"));
            index += 1;
        }
        if let Err(error) = std::fs::File::create(&path) {
            log::error!("new file create failed: {error}");
            return;
        }
        self.file_browser.reveal_dir(&dir, &dir);
        self.reindex(dir.clone(), true, cx);
        self.begin_rename(
            RenameTarget::File {
                path,
                created: true,
            },
            "untitled".to_string(),
            cx,
        );
    }

    /// Create a new empty directory (path prompt under `dir`).
    pub(crate) fn new_folder_in_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_new_path(&dir, Some("untitled"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |this, cx| {
                    if let Err(error) = std::fs::create_dir_all(&path) {
                        log::error!("new folder failed: {error}");
                        return;
                    }
                    if let Some(id) = this.active
                        && let Some(root) = this.workspace_root(id)
                    {
                        this.file_browser.reveal_dir(&root, &path);
                        this.reindex(root, true, cx);
                    }
                    cx.notify();
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
