use super::{Content, EditorView, entries};
use crate::item_editor::ItemEditor;
use crate::worklist_file::{Change, WorkItem};
use anyhow::{Result, anyhow};
use gpui::{AppContext, Context};
use std::ops::Range;

impl EditorView {
    pub fn worklist_source(&self) -> Option<String> {
        match &self.content {
            Content::Text(buffer) => Some(buffer.text()),
            _ => None,
        }
    }

    pub(in crate::view) fn worklist_mutate(
        &mut self,
        range: Range<usize>,
        replacement: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(source) = self.worklist_source() else {
            return false;
        };
        let start = source[..range.start].chars().count();
        let end = source[..range.end].chars().count();
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        buffer.replace_range(start..end, replacement);
        let root = buffer
            .path()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let saved = match buffer.save_worklist(&root) {
            Ok(()) => {
                self.worklist_error = None;
                self.recompute_highlights();
                self.emit_buffer_changed(cx);
                true
            }
            Err(error) => {
                buffer.undo();
                self.worklist_error = Some(format!("Worklist change was not saved: {error}"));
                false
            }
        };
        cx.notify();
        saved
    }

    /// Lands a change made outside this view through this buffer, so an open
    /// worklist has one writer. Returns the source after the change.
    pub fn worklist_apply(&mut self, change: &Change, cx: &mut Context<Self>) -> Result<String> {
        if self.is_dirty() {
            return Err(anyhow!("Save or undo the worklist Markdown edits first"));
        }
        if !self.worklist_mutate(change.range.clone(), &change.text, cx) {
            let error = self.worklist_error.take();
            cx.notify();
            return Err(anyhow!(
                error.unwrap_or_else(|| "Worklist is not editable".into())
            ));
        }
        Ok(self.text())
    }

    pub(super) fn worklist_toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        let source = self.text();
        let Some(entry) = entries(&source).get(index).cloned() else {
            return;
        };
        let Some(checked) = entry.checked else { return };
        if !entry.editable {
            return;
        }
        let raw = &source[entry.range.clone()];
        let Some(mark) = raw.find(if checked { "[x]" } else { "[ ]" }) else {
            return;
        };
        let at = entry.range.start + mark + 1;
        self.worklist_mutate(at..at + 1, if checked { " " } else { "x" }, cx);
    }

    pub(super) fn worklist_start_edit(&mut self, index: usize, cx: &mut Context<Self>) {
        let source = self.text();
        let Some(entry) = entries(&source).get(index).cloned() else {
            return;
        };
        if !entry.editable {
            return;
        }
        self.worklist_selection = index;
        let editor = cx.new(|cx| ItemEditor::existing(&source, &entry, cx));
        self.worklist_host(editor, cx);
    }

    pub(super) fn worklist_land(&mut self, item: &WorkItem, cx: &mut Context<Self>) {
        let Some(editor) = self.worklist_edit.clone() else {
            return;
        };
        let mut draft = editor.read(cx).draft();
        let change = match draft.save(&self.text(), item) {
            Ok(Some(change)) => change,
            Ok(None) => return,
            Err(error) => {
                self.worklist_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if self.worklist_mutate(change.range.clone(), &change.text, cx) {
            if let Some(index) = draft.commit(&self.text(), &change) {
                self.worklist_selection = index;
            }
            editor.update(cx, |editor, cx| editor.landed(draft, item.clone(), cx));
        }
    }

    pub(super) fn worklist_close_edit(&mut self, cx: &mut Context<Self>) {
        self.worklist_edit = None;
        self.worklist_input_sub.clear();
        self.worklist_error = None;
        cx.notify();
    }

    pub(super) fn worklist_delete(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.worklist_edit.clone() else {
            return;
        };
        let change = match editor.read(cx).draft().remove(&self.text()) {
            Ok(Some(change)) => change,
            Ok(None) => return self.worklist_close_edit(cx),
            Err(error) => {
                self.worklist_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if self.worklist_mutate(change.range.clone(), &change.text, cx) {
            self.worklist_close_edit(cx);
        }
    }
}
