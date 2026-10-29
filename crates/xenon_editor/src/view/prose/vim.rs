//! Vim in prose mode. The vim layer edits the one buffer as it does in source;
//! insert-mode keys keep prose behavior (list continuation, marker-aware
//! backspace), and bare `j`/`k` move by display row, since one source line is
//! a whole paragraph here.

use gpui::{Context, KeyDownEvent};

use super::super::{Content, EditorView};
use crate::vim::Mode;

impl EditorView {
    /// Returns true when vim took the key; false lets prose handle it.
    pub(in crate::view) fn prose_vim_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let k = &event.keystroke;
        let drafting = self.vim.search_draft.is_some() || self.vim.ex_draft.is_some();
        if self.vim.mode == Mode::Insert && !drafting && k.key != "escape" {
            return false;
        }
        if k.modifiers.platform && !drafting {
            return false;
        }
        let plain = !k.modifiers.modified();
        if plain && self.vim.bare_normal() && matches!(k.key.as_str(), "up" | "down") {
            self.prose_rows(if k.key == "up" { -1 } else { 1 }, cx);
            cx.stop_propagation();
            return true;
        }
        let taken = self.route_vim_keystroke(k, cx);
        self.prose_vim_settle(cx);
        taken
    }

    /// Typed text with vim on. Returns true when vim consumed it.
    pub(in crate::view) fn prose_vim_char(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        if self.vim.bare_normal() && matches!(text, "j" | "k") {
            self.prose_rows(if text == "k" { -1 } else { 1 }, cx);
            return true;
        }
        if !self.handle_vim_char(text, cx) {
            return false;
        }
        self.prose_vim_settle(cx);
        true
    }

    fn prose_rows(&mut self, dir: i32, cx: &mut Context<Self>) {
        for _ in 0..self.vim.count_for_ctrl().max(1) {
            self.vertical(dir, false, cx);
        }
    }

    /// After a vim command: drop the column goal and, in Normal mode, move a
    /// caret left inside hidden or collapsed source to the next visible stop.
    fn prose_vim_settle(&mut self, cx: &mut Context<Self>) {
        self.prose.goal_x = None;
        if self.vim.mode != Mode::Normal {
            return;
        }
        let Some(proj) = self.fresh_projection(cx) else {
            return;
        };
        let len = self.rope_len();
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let pos = buffer.cursor();
        if !proj.valid(pos) {
            buffer.set_cursor_raw(proj.step(pos, 1, len));
            self.emit_cursor(cx);
        }
    }
}
