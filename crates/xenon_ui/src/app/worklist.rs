use super::*;
use crate::worklist_capture::{CaptureEvent, WorklistCaptureView};
use xenon_editor::worklist_file::WorklistFile;

impl XenonApp {
    pub(super) fn show_worklist_notice(
        &mut self,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.worklist_notice
            .show(message, cx, |app, generation, cx| {
                if app.worklist_notice.expire(generation) {
                    cx.notify();
                }
            });
    }

    pub(super) fn dismiss_worklist_notice(&mut self, cx: &mut Context<Self>) {
        self.worklist_notice.dismiss();
        cx.notify();
    }

    pub(super) fn bind_worklist_actions(
        &self,
        root: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        root.on_action(cx.listener(|this, _: &crate::CaptureWorklist, window, cx| {
            this.capture_worklist(window, cx);
        }))
        .on_action(cx.listener(|this, _: &crate::OpenWorklist, _, cx| {
            this.open_worklist(cx);
        }))
    }

    pub(crate) fn capture_worklist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.active else {
            self.show_worklist_notice("Open a workspace first", cx);
            return;
        };
        if self.worklist_capture_visible == Some(workspace) {
            self.worklist_capture_visible = None;
            self.focus_after_teardown(Some(window), cx);
            cx.notify();
            return;
        }
        self.deferred.restore_pane = Some(self.current_focus_owner(window, cx));
        let capture = if let Some(capture) = self.worklist_captures.get(&workspace) {
            capture.clone()
        } else {
            let name = self
                .registry
                .workspace(workspace)
                .map_or_else(|| "Workspace".to_string(), |w| w.name.clone());
            let capture = cx.new(|cx| WorklistCaptureView::new(name, cx));
            let subscription = cx.subscribe(&capture, move |this, _, event, cx| {
                this.handle_worklist_capture(workspace, event, cx);
            });
            self.worklist_capture_subs.push(subscription);
            self.worklist_captures.insert(workspace, capture.clone());
            capture
        };
        self.worklist_capture_visible = Some(workspace);
        capture.update(cx, |view, cx| view.open(cx));
        cx.notify();
    }

    fn handle_worklist_capture(
        &mut self,
        workspace: WorkspaceId,
        event: &CaptureEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            CaptureEvent::Dismissed => {
                self.worklist_capture_visible = None;
                self.deferred.pending_focus = self.deferred.restore_pane.take();
            }
            CaptureEvent::Submit { text } => {
                let Some(root) = self.workspace_root(workspace) else {
                    return;
                };
                let Some(capture) = self.worklist_captures.get(&workspace).cloned() else {
                    return;
                };
                let worklist_path = root.join(".xenon/worklist.md");
                let editor = self
                    .contents
                    .get(&workspace)
                    .and_then(|content| content.root.as_ref())
                    .and_then(|tree| tree.find_editor_path(&worklist_path))
                    .and_then(|(pane, index)| {
                        self.contents
                            .get(&workspace)?
                            .root
                            .as_ref()?
                            .find_leaf(pane)?
                            .tabs
                            .get(index)?
                            .as_editor()
                    })
                    .cloned();
                if editor
                    .as_ref()
                    .is_some_and(|editor| editor.read(cx).is_dirty())
                {
                    capture.update(cx, |view, cx| {
                        view.failed(
                            "Save or cancel the worklist edits first. Open Worklist to continue."
                                .into(),
                            cx,
                        )
                    });
                    return;
                }
                match WorklistFile::new(&root).and_then(|file| file.append_with_undo(text, true)) {
                    Ok((_, undo)) => {
                        capture.update(cx, |view, cx| view.saved(cx));
                        self.worklist_undo = Some((workspace, undo));
                        self.show_worklist_notice("Task added", cx);
                        if let Some(editor) = editor {
                            editor
                                .update(cx, |editor, cx| editor.refresh_worklist_after_capture(cx));
                        }
                        if self.worklist_capture_visible == Some(workspace) {
                            self.worklist_capture_visible = None;
                        }
                        self.deferred.pending_focus = self.deferred.restore_pane.take();
                    }
                    Err(error) => capture.update(cx, |view, cx| view.failed(error.to_string(), cx)),
                }
            }
        }
        cx.notify();
    }

    pub(super) fn undo_last_worklist_capture(&mut self, cx: &mut Context<Self>) {
        let Some((workspace, undo)) = self.worklist_undo.take() else {
            return;
        };
        match undo.undo() {
            Ok(()) => {
                self.show_worklist_notice("Capture undone", cx);
                let path = self
                    .workspace_root(workspace)
                    .map(|root| root.join(".xenon/worklist.md"));
                if let Some(path) = path
                    && let Some(editor) = self
                        .contents
                        .get(&workspace)
                        .and_then(|c| c.root.as_ref())
                        .and_then(|tree| tree.find_editor_path(&path))
                        .and_then(|(pane, index)| {
                            self.contents
                                .get(&workspace)?
                                .root
                                .as_ref()?
                                .find_leaf(pane)?
                                .tabs
                                .get(index)?
                                .as_editor()
                        })
                {
                    editor.update(cx, |editor, cx| editor.refresh_worklist_after_capture(cx));
                }
            }
            Err(error) => self.show_worklist_notice(format!("Undo paused: {error}"), cx),
        }
        cx.notify();
    }
}
