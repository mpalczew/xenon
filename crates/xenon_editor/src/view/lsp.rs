use std::ops::Range;
use std::path::PathBuf;

use gpui::Context;

use super::{Content, EditorDiagnostic, EditorEvent, EditorView};

impl EditorView {
    pub fn text_snapshot(&self) -> Option<(PathBuf, String)> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        Some((buffer.path().to_path_buf(), buffer.text()))
    }

    pub fn cursor_position(&self) -> Option<(u32, u32)> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let (row, col) = buffer.cursor_position();
        Some((row as u32, col as u32))
    }

    pub fn set_cursor_position(&mut self, row: u32, col: u32, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.set_cursor_position(row as usize, col as usize);
        self.emit_cursor(cx);
        cx.notify();
    }

    pub fn line_text(&self, row: u32) -> Option<String> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let row = row as usize;
        (row < buffer.rope().len_lines()).then(|| buffer.rope().line(row).to_string())
    }

    pub fn char_offset(&self, row: u32, col: u32) -> Option<usize> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let row = row as usize;
        if row >= buffer.rope().len_lines() {
            return None;
        }
        let line = buffer.rope().line(row);
        Some(buffer.rope().line_to_char(row) + (col as usize).min(line.len_chars()))
    }

    pub fn set_lsp_decorations(
        &mut self,
        occurrences: Vec<Range<usize>>,
        diagnostics: Vec<EditorDiagnostic>,
        cx: &mut Context<Self>,
    ) {
        self.occurrence_ranges = occurrences;
        self.diagnostics = diagnostics;
        cx.notify();
    }

    pub fn set_lsp_status(&mut self, status: Option<String>, cx: &mut Context<Self>) {
        if self.lsp_status != status {
            self.lsp_status = status;
            cx.notify();
        }
    }

    pub(super) fn emit_buffer_changed(&mut self, cx: &mut Context<Self>) {
        let Some((path, text)) = self.text_snapshot() else {
            return;
        };
        self.occurrence_ranges.clear();
        self.diagnostics.clear();
        cx.emit(EditorEvent::BufferChanged { path, text });
    }

    pub(super) fn emit_cursor(&mut self, cx: &mut Context<Self>) {
        let Some((row, col)) = self.cursor_position() else {
            return;
        };
        cx.emit(EditorEvent::CursorMoved {
            path: self.path().to_path_buf(),
            row,
            col,
        });
    }

    pub(super) fn emit_go_to_definition(&mut self, cx: &mut Context<Self>) {
        let Some((row, col)) = self.cursor_position() else {
            return;
        };
        cx.emit(EditorEvent::GoToDefinition {
            path: self.path().to_path_buf(),
            row,
            col,
        });
    }
}
