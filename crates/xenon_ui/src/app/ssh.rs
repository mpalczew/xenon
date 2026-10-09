use super::*;

impl XenonApp {
    pub(super) fn terminal_workspace_path(
        &mut self,
        owner: Option<(WorkspaceId, TabId)>,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Option<PathBuf> {
        let Some(record) = owner.and_then(|(id, _)| self.registry.workspace(id)) else {
            return Some(path.to_path_buf());
        };
        let Some(ssh) = &record.ssh else {
            return Some(path.to_path_buf());
        };
        let relative = if path.is_absolute() {
            path.strip_prefix(&ssh.directory).ok()
        } else {
            Some(path)
        };
        if let Some(relative) = relative {
            return Some(record.root.join(relative));
        }
        self.show_toast(
            super::toasts::failed("That path is outside the SSH workspace", path.display()),
            cx,
        );
        None
    }
    pub(super) fn connect_ssh_workspace(&mut self, address: &str, cx: &mut Context<Self>) {
        let ssh = match xenon_ssh::SshWorkspace::parse(address) {
            Ok(ssh) => ssh,
            Err(error) => {
                self.show_toast(super::toasts::failed("Couldn’t connect", error), cx);
                return;
            }
        };
        self.workspace_picker = None;
        self.deferred.pending_focus = self.deferred.restore_pane.take();
        cx.spawn(async move |app, cx| {
            let mut connected = ssh.clone();
            let result = cx
                .background_executor()
                .spawn(async move { ssh.probe() })
                .await;
            app.update(cx, |app, cx| match result {
                Ok(directory) => {
                    connected.directory = directory;
                    if let Some(record) = app
                        .registry
                        .workspaces
                        .iter()
                        .find(|record| record.ssh.as_ref() == Some(&connected))
                    {
                        app.activate_workspace(record.id, cx);
                        return;
                    }
                    if let Some(record) = app
                        .registry
                        .closed_workspaces
                        .iter()
                        .find(|record| record.ssh.as_ref() == Some(&connected))
                    {
                        app.reopen_workspace(record.id, cx);
                        return;
                    }
                    let record = WorkspaceRec::remote(connected);
                    let id = record.id;
                    app.registry.workspaces.push(record);
                    save_registry(&app.registry, "connect_ssh_workspace");
                    app.activate_workspace(id, cx);
                    app.focus_workspace_leaf(None, cx);
                }
                Err(error) => app.show_toast(
                    super::toasts::failed("Couldn’t connect over SSH", error),
                    cx,
                ),
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn ssh_for_root(&self, root: &Path) -> Option<xenon_ssh::SshWorkspace> {
        self.registry
            .workspaces
            .iter()
            .find(|record| record.root == root)
            .and_then(|record| record.ssh.clone())
    }

    pub(super) fn ssh_reindex(
        &mut self,
        root: PathBuf,
        ssh: xenon_ssh::SshWorkspace,
        cx: &mut Context<Self>,
    ) {
        let key = root.clone();
        let task = cx.spawn(async move |app, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { ssh.entries() })
                .await;
            app.update(cx, |app, cx| {
                app.index_tasks.remove(&root);
                if !app
                    .registry
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.root == root)
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        let index = FileIndex::from_paths(
                            entries
                                .iter()
                                .map(|entry| (entry.path.clone(), entry.is_dir)),
                        );
                        app.file_browser.install_remote(root.clone(), entries);
                        app.install_index(root, Arc::new(index), true, cx);
                    }
                    Err(error) => app.show_toast(
                        super::toasts::failed("Couldn’t refresh remote files", error),
                        cx,
                    ),
                }
                cx.notify();
            })
            .ok();
        });
        self.index_tasks.insert(key, task);
    }
}
