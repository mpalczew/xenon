//! Ex / save-as disk side effects for the editor view.

use std::path::PathBuf;

use gpui::{ClipboardItem, Context};

use super::{Content, EditorEvent, EditorView};
use crate::buffer::SaveError;
use crate::vim::ExEffect;

impl EditorView {
    pub(super) fn apply_ex_effect(&mut self, effect: ExEffect, cx: &mut Context<Self>) {
        match effect {
            ExEffect::Write { force, path } => {
                self.ex_write(force, path, cx);
            }
            ExEffect::WriteQuit { force } => {
                if self.ex_write(force, None, cx) {
                    cx.emit(EditorEvent::RequestClose { force: true });
                }
            }
            ExEffect::Reload { force } => {
                let Content::Text(_) = &self.content else {
                    return;
                };
                if force || !self.is_dirty() {
                    self.reload_from_disk(cx);
                    self.vim.ex_status = Some(format!("\"{}\"", self.path().display()));
                } else {
                    self.vim.ex_status =
                        Some("No write since last change (add ! to override)".into());
                    cx.notify();
                }
            }
            ExEffect::Quit { force } => {
                if !force && self.is_dirty() {
                    self.vim.ex_status =
                        Some("No write since last change (add ! to override)".into());
                    cx.notify();
                    return;
                }
                cx.emit(EditorEvent::RequestClose { force });
            }
            ExEffect::Message(msg) => {
                self.vim.ex_status = if msg.is_empty() { None } else { Some(msg) };
                cx.notify();
            }
            ExEffect::Error(msg) => {
                self.vim.ex_status = Some(msg);
                cx.notify();
            }
            ExEffect::Edited { message } => {
                self.vim.ex_status = Some(message);
                self.recompute_highlights();
                cx.notify();
            }
        }
    }

    /// Write buffer; optionally rebind path. Returns whether write succeeded.
    fn ex_write(&mut self, force: bool, path: Option<PathBuf>, cx: &mut Context<Self>) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        if let Some(new_path) = path {
            buffer.set_path(new_path.clone());
            cx.emit(EditorEvent::PathChanged { path: new_path });
        }
        let result = if force {
            buffer.save_force()
        } else {
            buffer.save()
        };
        match result {
            Ok(()) => {
                let p = buffer.path().display().to_string();
                self.vim.ex_status = Some(format!("\"{p}\" written"));
                self.disk_alert = super::DiskAlert::None;
                self.recompute_highlights();
                cx.notify();
                true
            }
            Err(SaveError::ExternalChange) => {
                self.disk_alert = super::DiskAlert::Conflict;
                self.vim.ex_status = Some("File changed on disk (use :w! to override)".into());
                cx.notify();
                false
            }
            Err(SaveError::Io(e)) => {
                self.vim.ex_status = Some(format!("write failed: {e}"));
                cx.notify();
                false
            }
        }
    }

    pub(super) fn undo_edit(&mut self) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        buffer.undo()
    }

    pub(super) fn redo_edit(&mut self) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        buffer.redo()
    }

    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        if self.preview {
            let text = self.preview_state.read(cx).selected_text();
            if !text.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            return;
        }
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let text = buffer.selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub fn cut_selection(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            // Read-only preview: cut degenerates to copy.
            self.copy_selection(cx);
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let text = buffer.selected_text();
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        buffer.delete_selection();
        self.recompute_highlights();
    }

    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.replace_selection(&text);
        self.recompute_highlights();
    }

    /// Write the buffer to disk (no-op for image/unsupported content).
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        match buffer.save() {
            Ok(()) => {
                self.disk_alert = super::DiskAlert::None;
            }
            Err(SaveError::ExternalChange) => {
                self.disk_alert = super::DiskAlert::Conflict;
                log::warn!("save refused: file changed on disk");
            }
            Err(error) => log::error!("save failed: {error}"),
        }
        cx.notify();
    }

    /// Force write (overwrite external changes).
    pub fn save_force(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if let Err(error) = buffer.save_force() {
            log::error!("force save failed: {error}");
        } else {
            self.disk_alert = super::DiskAlert::None;
        }
        cx.notify();
    }

    /// Save As: rebind path and write.
    pub fn save_as(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.set_path(path.clone());
        if let Err(error) = buffer.save_force() {
            log::error!("save as failed: {error}");
            return;
        }
        self.disk_alert = super::DiskAlert::None;
        self.recompute_highlights();
        cx.emit(EditorEvent::PathChanged { path });
        cx.notify();
    }
}
