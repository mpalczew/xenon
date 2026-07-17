//! Keyboard, mouse, and clipboard input for the text editor.

use gpui::{
    ClipboardItem, Context, KeyDownEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window,
};

use super::{Content, EditorView};
use crate::edit::{EditCommand, Motion};
use crate::mouse;
use crate::vim::Mode;

impl EditorView {
    pub(super) fn on_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        // Find bar owns keys when its input is focused.
        if self.find_bar_focused(window) {
            return;
        }
        // Esc closes find when editor body is focused.
        if keystroke.key == "escape" && self.find_is_open() {
            self.close_find(window, cx);
            cx.stop_propagation();
            return;
        }
        if matches!(self.content, Content::Image(_)) {
            if !keystroke.modifiers.platform {
                return;
            }
            match keystroke.key.as_str() {
                "=" | "+" => self.zoom_image_in(cx),
                "-" => self.zoom_image_out(cx),
                "0" => self.fit_image(cx),
                "1" => self.actual_size_image(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if !matches!(self.content, Content::Text(_)) {
            return;
        }
        if keystroke.modifiers.platform && keystroke.key == "z" {
            let edited = if keystroke.modifiers.shift {
                self.redo_edit()
            } else {
                self.undo_edit()
            };
            if edited {
                self.recompute_highlights();
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if keystroke.modifiers.platform && keystroke.key == "a" {
            self.select_all(cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.modifiers.control && keystroke.key == "r" {
            if self.redo_edit() {
                self.recompute_highlights();
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if xero_settings::vim_mode(cx) && self.route_vim_keystroke(keystroke, cx) {
            return;
        }
        let extend = keystroke.modifiers.shift;
        let Some(command) = command_for(keystroke, extend) else {
            return;
        };
        let edits = command.edits();
        if let Content::Text(buffer) = &mut self.content {
            buffer.apply(command);
        }
        if edits {
            self.recompute_highlights();
        } else {
            // Move/Extend: push selection to Claude IDE bridge.
            self.emit_selection(cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn handle_vim_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        let result = self.vim.handle_key(buffer, key);
        if result.request_system_paste {
            if let Some(item) = cx.read_from_clipboard()
                && let Some(text) = item.text()
            {
                let r = self.vim.paste_system(buffer, &text, false);
                if r.edited {
                    self.recompute_highlights();
                }
            }
            return true;
        }
        if let Some(text) = result.system_clipboard {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
        if result.edited {
            self.recompute_highlights();
        }
        result.handled
    }

    /// Returns true if the keystroke was fully consumed (caller should return).
    fn route_vim_keystroke(&mut self, keystroke: &gpui::Keystroke, cx: &mut Context<Self>) -> bool {
        // `/` search: take printable key_char here. Claiming "handled" on
        // KeyDown without consuming key_char blocks IME insertText on macOS.
        if self.vim.search_draft.is_some() {
            let mods = &keystroke.modifiers;
            if !mods.platform
                && !mods.control
                && !mods.function
                && let Some(ch) = keystroke.key_char.as_deref()
                && ch != "\n"
                && ch != "\r"
                && !ch.is_empty()
                && self.handle_vim_char(ch, cx)
            {
                cx.stop_propagation();
                cx.notify();
                return true;
            }
            if self.handle_vim_key(&keystroke.key, cx) {
                cx.stop_propagation();
                cx.notify();
                return true;
            }
            // Stay in search prompt; don't fall through to buffer edits.
            return true;
        }
        if self.handle_vim_key(&keystroke.key, cx) {
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        // Insert-mode editing keys still use EditCommand.
        self.vim.mode != Mode::Insert
    }

    pub(super) fn handle_vim_char(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        let result = self.vim.handle_char(buffer, text);
        if result.request_system_paste {
            if let Some(item) = cx.read_from_clipboard()
                && let Some(text) = item.text()
            {
                let r = self.vim.paste_system(buffer, &text, false);
                if r.edited {
                    self.recompute_highlights();
                }
            }
            return true;
        }
        if let Some(clip) = result.system_clipboard {
            cx.write_to_clipboard(ClipboardItem::new_string(clip));
        }
        if result.edited {
            self.recompute_highlights();
        }
        // If vim consumed the key as a command (e.g. `i` entering insert), do not
        // also insert that character. Only plain typing in insert returns false.
        result.handled
    }

    fn undo_edit(&mut self) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        buffer.undo()
    }

    fn redo_edit(&mut self) -> bool {
        let Content::Text(buffer) = &mut self.content else {
            return false;
        };
        buffer.redo()
    }

    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let text = buffer.selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub fn cut_selection(&mut self, cx: &mut Context<Self>) {
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
        if let Err(error) = buffer.save() {
            log::error!("save failed: {error}");
        }
        cx.notify();
    }

    pub(super) fn on_right_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(event.position);
        self.focus.focus(window, cx);
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn dismiss_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !mouse::is_primary_down(event) {
            return;
        }
        self.dismiss_menu(cx);
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let Some(layout) = self.click_layout else {
            return;
        };
        let (row, col) = mouse::position_at(layout, event.position);
        let click_count = self.click_tracker.count(row, col);
        let extend = event.modifiers.shift;
        match click_count {
            2 => {
                buffer.set_cursor_position(row, col);
                let cursor = buffer.cursor();
                buffer.select_word_at(cursor);
                self.dragging = false;
            }
            3 => {
                buffer.set_cursor_position(row, col);
                let cursor = buffer.cursor();
                buffer.select_line_at(cursor);
                self.dragging = false;
            }
            _ if extend => {
                buffer.set_cursor_position_extend(row, col, true);
                self.dragging = true;
            }
            _ => {
                buffer.set_cursor_position(row, col);
                buffer.set_selection(buffer.cursor(), buffer.cursor());
                self.dragging = true;
            }
        }
        // Mouse selection drops visual mode back to normal if vim is on.
        if xero_settings::vim_mode(cx) && self.vim.mode.is_visual() {
            self.vim.mode = Mode::Normal;
        }
        // Double/triple click finish selection without a drag; emit now.
        if click_count >= 2 {
            self.emit_selection(cx);
        }
        self.focus.focus(window, cx);
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging || !mouse::is_left_drag(event) {
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let Some(layout) = self.click_layout else {
            return;
        };
        let (row, col) = mouse::position_at(layout, event.position);
        buffer.set_cursor_position_extend(row, col, true);
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn on_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dragging {
            self.dragging = false;
            // Collapse empty drag selections.
            if let Content::Text(buffer) = &mut self.content
                && buffer.selection_range().is_none()
            {
                buffer.clear_selection();
            }
            self.emit_selection(cx);
            cx.notify();
        }
    }
}

fn command_for(keystroke: &gpui::Keystroke, extend: bool) -> Option<EditCommand> {
    // ⌘/⌥ navigation common on macOS (line / file / word).
    let motion = if keystroke.modifiers.platform {
        match keystroke.key.as_str() {
            "left" => Some(Motion::LineStart),
            "right" => Some(Motion::LineEnd),
            "up" => Some(Motion::FileStart),
            "down" => Some(Motion::FileEnd),
            _ => None,
        }
    } else if keystroke.modifiers.alt {
        match keystroke.key.as_str() {
            "left" => Some(Motion::WordLeft),
            "right" => Some(Motion::WordRight),
            _ => None,
        }
    } else {
        None
    };
    if let Some(motion) = motion {
        return Some(if extend {
            EditCommand::Extend(motion)
        } else {
            EditCommand::Move(motion)
        });
    }
    let key = keystroke.key.as_str();
    Some(match key {
        "backspace" => EditCommand::Backspace,
        "delete" => EditCommand::Delete,
        "enter" => EditCommand::Newline,
        "tab" => EditCommand::Insert("    ".into()),
        "left" if extend => EditCommand::Extend(Motion::Left),
        "right" if extend => EditCommand::Extend(Motion::Right),
        "up" if extend => EditCommand::Extend(Motion::Up),
        "down" if extend => EditCommand::Extend(Motion::Down),
        "home" if extend => EditCommand::Extend(Motion::LineStart),
        "end" if extend => EditCommand::Extend(Motion::LineEnd),
        "left" => EditCommand::Move(Motion::Left),
        "right" => EditCommand::Move(Motion::Right),
        "up" => EditCommand::Move(Motion::Up),
        "down" => EditCommand::Move(Motion::Down),
        "home" => EditCommand::Move(Motion::LineStart),
        "end" => EditCommand::Move(Motion::LineEnd),
        _ => return None,
    })
}
