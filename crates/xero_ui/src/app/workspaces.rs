use super::*;

impl XeroApp {
    pub(super) fn add_workspace(&mut self, cx: &mut Context<Self>) {
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
        if let Some(record) = self
            .registry
            .closed_workspaces
            .iter()
            .find(|record| record.root == root)
        {
            self.reopen_workspace(record.id, cx);
            return;
        }
        let mut record = WorkspaceRec::new(root);
        let mut stream = Stream::new("stream 1");
        stream.session.layout = self.current_layout();
        let stream_id = stream.id;
        record.streams.push(stream_id);
        let workspace_id = record.id;
        self.registry.workspaces.push(record);
        self.streams.insert(stream_id, stream.clone());
        save_session(workspace_id, &stream, "register_workspace");
        save_registry(&self.registry, "register_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        self.activate_stream(stream_id, cx);
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

    /// Drop workspace streams with no dirty check. `window` restores focus when present.
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
        let closed_active = self
            .active
            .is_some_and(|active| record.streams.contains(&active));
        for stream in &record.streams {
            self.streams.remove(stream);
            self.terminals.remove(stream);
            self.editors.remove(stream);
            self.attention.remove(stream);
        }
        self.collapsed_workspaces.remove(&id);
        self.git_dirt.remove(&id);
        self.registry.closed_workspaces.push(record);
        if closed_active {
            self.active = None;
            self.registry.active = None;
        }
        save_registry(&self.registry, "close_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        if closed_active && let Some(next) = self.first_stream() {
            match window {
                Some(window) => self.select_stream(next, window, cx),
                None => self.activate_stream(next, cx),
            }
        }
        cx.notify();
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
        let mut record = self.registry.closed_workspaces.remove(index);
        if record.streams.is_empty() {
            let mut stream = Stream::new("main");
            stream.session.layout = self.current_layout();
            record.streams.push(stream.id);
            save_session(record.id, &stream, "reopen_workspace default stream");
            self.streams.insert(stream.id, stream);
        } else {
            for &stream in &record.streams {
                let loaded = xero_store::load_session(record.id, stream)
                    .unwrap_or_else(|_| synthesize_stream(stream));
                self.streams.insert(stream, loaded);
            }
        }
        let first = record.streams.first().copied();
        self.registry.workspaces.push(record);
        save_registry(&self.registry, "reopen_workspace");
        self.update_ide_roots();
        self.restart_git_dirt_watch(cx);
        if let Some(stream) = first {
            self.activate_stream(stream, cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn toggle_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if !self.collapsed_workspaces.insert(id) {
            self.collapsed_workspaces.remove(&id);
        }
        cx.notify();
    }

    pub(crate) fn is_workspace_collapsed(&self, id: WorkspaceId) -> bool {
        self.collapsed_workspaces.contains(&id)
    }

    pub(crate) fn closed_section_collapsed(&self) -> bool {
        self.closed_section_collapsed
    }

    pub(crate) fn toggle_closed_section(&mut self, cx: &mut Context<Self>) {
        self.closed_section_collapsed = !self.closed_section_collapsed;
        cx.notify();
    }
}
