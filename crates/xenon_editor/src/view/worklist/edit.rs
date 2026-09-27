use super::{Content, EditorView, ItemEdit, entries, source::replacement};
use crate::EditorEvent;
use gpui::Context;
use std::ops::Range;

impl EditorView {
    fn worklist_source(&self) -> Option<String> {
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
        self.worklist_capture = None;
        self.worklist_input_sub.clear();
        let Some(entry) = entries(&self.text()).get(index).cloned() else {
            return;
        };
        if !entry.editable {
            return;
        }
        self.worklist_selection = index;
        let form = self.worklist_new_form(&entry.title, &entry.details, entry.checked, cx);
        self.worklist_edit = Some(ItemEdit { form, entry });
        cx.notify();
    }

    pub(super) fn worklist_autosave_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.worklist_edit.as_ref() else {
            return;
        };
        let Ok(item) = edit.form.item(cx) else {
            return;
        };
        let newline = if self.text().contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let source = self.text();
        let range = edit.entry.range.clone();
        let old = &source[range.clone()];
        let suffix = &old[old.trim_end_matches(['\r', '\n']).len()..];
        let changed = format!("{}{}", replacement(&edit.entry, &item, newline), suffix);
        if changed == old {
            return;
        }
        let index = self.worklist_selection;
        if self.worklist_mutate(range, &changed, cx)
            && let Some(entry) = entries(&self.text()).get(index).cloned()
            && let Some(edit) = &mut self.worklist_edit
        {
            edit.entry = entry;
        }
    }

    pub(super) fn worklist_close_edit(&mut self, cx: &mut Context<Self>) {
        self.worklist_edit = None;
        self.worklist_input_sub.clear();
        self.worklist_error = None;
        cx.notify();
    }

    pub(super) fn worklist_delete(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.worklist_edit.take() else {
            return;
        };
        if !self.worklist_mutate(edit.entry.range.clone(), "", cx) {
            self.worklist_edit = Some(edit);
        } else {
            self.worklist_input_sub.clear();
        }
    }

    pub(super) fn worklist_undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let changed = if redo { buffer.redo() } else { buffer.undo() };
        if !changed {
            if !redo {
                cx.emit(EditorEvent::RequestWorklistUndo);
            }
            return;
        }
        let root = buffer
            .path()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        if let Err(error) = buffer.save_worklist(&root) {
            if redo {
                buffer.undo();
            } else {
                buffer.redo();
            }
            self.worklist_error = Some(format!("Undo was not saved: {error}"));
        } else {
            self.worklist_error = None;
            self.recompute_highlights();
            self.emit_buffer_changed(cx);
        }
        cx.notify();
    }
}
