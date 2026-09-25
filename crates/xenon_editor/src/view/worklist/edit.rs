use super::{Content, EditorView, ItemEdit, entries, source::replacement};
use crate::EditorEvent;
use gpui::Context;
use gpui::px;
use std::ops::Range;
use xenon_design_system::TextInputConfig;

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
        self.worklist_input_sub = None;
        let Some(entry) = entries(&self.text()).get(index).cloned() else {
            return;
        };
        if !entry.editable {
            return;
        }
        self.worklist_selection = index;
        let input = self.worklist_new_input(
            TextInputConfig::multiline("Edit item", px(180.)),
            &entry.text,
            cx,
        );
        self.worklist_edit = Some(ItemEdit { input, entry });
        cx.notify();
    }

    pub(super) fn worklist_save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.worklist_edit.as_ref() else {
            return;
        };
        let text = edit.input.read(cx).text().to_owned();
        self.worklist_save_edit_text(&text, cx);
    }

    pub(super) fn worklist_save_edit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(edit) = self.worklist_edit.take() else {
            return;
        };
        let text = text.trim();
        if text.is_empty() {
            self.worklist_edit = Some(edit);
            return;
        }
        let newline = if self.text().contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let source = self.text();
        let old = &source[edit.entry.range.clone()];
        let suffix = &old[old.trim_end_matches(['\r', '\n']).len()..];
        let changed = format!("{}{}", replacement(&edit.entry, text, newline), suffix);
        if !self.worklist_mutate(edit.entry.range.clone(), &changed, cx) {
            self.worklist_edit = Some(edit);
        } else {
            self.worklist_input_sub = None;
        }
    }

    pub(super) fn worklist_cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.worklist_edit = None;
        self.worklist_input_sub = None;
        cx.notify();
    }

    pub(super) fn worklist_delete(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.worklist_edit.take() else {
            return;
        };
        if !self.worklist_mutate(edit.entry.range.clone(), "", cx) {
            self.worklist_edit = Some(edit);
        } else {
            self.worklist_input_sub = None;
        }
    }

    pub(super) fn worklist_move(&mut self, direction: isize, cx: &mut Context<Self>) {
        if self.worklist_edit.is_none() {
            return;
        }
        let source = self.text();
        let rows = entries(&source);
        let at = self.worklist_selection;
        let Some(other) = at.checked_add_signed(direction).filter(|i| *i < rows.len()) else {
            return;
        };
        let (a, b) = if at < other {
            (&rows[at], &rows[other])
        } else {
            (&rows[other], &rows[at])
        };
        if a.section != b.section || !source[a.range.end..b.range.start].trim().is_empty() {
            return;
        }
        let gap = &source[a.range.end..b.range.start];
        let changed = format!(
            "{}{}{}",
            &source[b.range.clone()],
            gap,
            &source[a.range.clone()]
        );
        let span = a.range.start..b.range.end;
        if !self.worklist_mutate(span, &changed, cx) {
            return;
        }
        self.worklist_selection = other;
        if let Some(entry) = entries(&self.text()).get(other).cloned() {
            self.worklist_input_sub = None;
            let input = self.worklist_new_input(
                TextInputConfig::multiline("Edit item", px(180.)),
                &entry.text,
                cx,
            );
            self.worklist_edit = Some(ItemEdit { input, entry });
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
