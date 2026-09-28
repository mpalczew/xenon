use super::*;
use crate::worklist_capture::{CaptureEvent, WorklistCaptureView};
use anyhow::Context as _;
use xenon_editor::worklist_file::{Change, ItemDraft, WorkItem, WorklistFile};

enum DraftEdit {
    Save(WorkItem),
    Remove,
}

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
            self.hide_worklist_capture(cx);
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

    /// Hides the capture overlay. A fully saved item gets an Undo notice.
    pub(super) fn hide_worklist_capture(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.worklist_capture_visible.take() else {
            return;
        };
        let Some(capture) = self.worklist_captures.get(&workspace).cloned() else {
            return;
        };
        if let Some(draft) = capture.update(cx, |view, cx| view.finish(cx)) {
            self.worklist_undo = Some((workspace, draft));
            self.show_worklist_notice("Task added", cx);
        }
    }

    fn handle_worklist_capture(
        &mut self,
        workspace: WorkspaceId,
        event: &CaptureEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(capture) = self.worklist_captures.get(&workspace).cloned() else {
            return;
        };
        match event {
            CaptureEvent::Close => {
                self.hide_worklist_capture(cx);
                self.deferred.pending_focus = self.deferred.restore_pane.take();
            }
            CaptureEvent::Discard => {
                let mut draft = capture.update(cx, |view, cx| view.reset(cx));
                if let Err(error) =
                    self.write_worklist_draft(workspace, &mut draft, DraftEdit::Remove, cx)
                {
                    self.show_worklist_notice(format!("Delete paused: {error}"), cx);
                }
                self.worklist_capture_visible = None;
                self.deferred.pending_focus = self.deferred.restore_pane.take();
            }
            CaptureEvent::Save { item } => {
                let mut draft = capture.read(cx).draft(cx);
                let edit = DraftEdit::Save(item.clone());
                match self.write_worklist_draft(workspace, &mut draft, edit, cx) {
                    Ok(()) => capture.update(cx, |view, cx| view.landed(draft, item.clone(), cx)),
                    Err(error) => capture.update(cx, |view, cx| view.failed(error.to_string(), cx)),
                }
            }
        }
        cx.notify();
    }

    pub(super) fn undo_last_worklist_capture(&mut self, cx: &mut Context<Self>) {
        let Some((workspace, mut draft)) = self.worklist_undo.take() else {
            return;
        };
        match self.write_worklist_draft(workspace, &mut draft, DraftEdit::Remove, cx) {
            Ok(()) => self.show_worklist_notice("Capture undone", cx),
            Err(error) => self.show_worklist_notice(format!("Undo paused: {error}"), cx),
        }
        cx.notify();
    }

    /// Writes through the open worklist tab when there is one, so the file has one writer.
    fn write_worklist_draft(
        &mut self,
        workspace: WorkspaceId,
        draft: &mut ItemDraft,
        edit: DraftEdit,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let root = self
            .workspace_root(workspace)
            .context("Workspace is gone")?;
        let change_for = |source: &str| -> anyhow::Result<Option<Change>> {
            match &edit {
                DraftEdit::Save(item) => draft.save(source, item),
                DraftEdit::Remove => draft.remove(source),
            }
        };
        if let Some(editor) = self.worklist_editor(workspace, &root) {
            let source = editor
                .read(cx)
                .worklist_source()
                .context("Worklist is not editable")?;
            let Some(change) = change_for(&source)? else {
                return Ok(());
            };
            let source = editor.update(cx, |editor, cx| editor.worklist_apply(&change, cx))?;
            draft.commit(&source, &change);
            return Ok(());
        }
        let file = WorklistFile::new(&root)?;
        let old = file.read()?;
        let source = match old.as_deref() {
            Some(bytes) => std::str::from_utf8(bytes).context("worklist is not UTF-8")?,
            None => "",
        };
        let Some(change) = change_for(source)? else {
            return Ok(());
        };
        let updated = change.apply(source);
        file.write(old.as_deref(), updated.as_bytes())?;
        draft.commit(&updated, &change);
        Ok(())
    }

    fn worklist_editor(
        &self,
        workspace: WorkspaceId,
        root: &std::path::Path,
    ) -> Option<Entity<EditorView>> {
        let tree = self.contents.get(&workspace)?.root.as_ref()?;
        let (pane, index) = tree.find_editor_path(&root.join(".xenon/worklist.md"))?;
        tree.find_leaf(pane)?.tabs.get(index)?.as_editor().cloned()
    }
}
