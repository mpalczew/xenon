//! EntityInputHandler for macOS typed text into the editor / find bar.
//!
//! Marked (IME) text is a char range in the one buffer, so it behaves the same
//! in the source and prose views: the composition is plain buffer text until
//! it is committed, and one composition is one undo step.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::{Content, EditorView};
use crate::edit::EditCommand;

/// UTF-16 code-unit range to a char range, clamped to the rope.
pub(super) fn utf16_to_chars(rope: &ropey::Rope, range: &Range<usize>) -> Range<usize> {
    let cu = rope.len_utf16_cu();
    let a = range.start.min(cu);
    let b = range.end.min(cu).max(a);
    rope.utf16_cu_to_char(a)..rope.utf16_cu_to_char(b)
}

pub(super) fn chars_to_utf16(rope: &ropey::Rope, range: &Range<usize>) -> Range<usize> {
    let len = rope.len_chars();
    rope.char_to_utf16_cu(range.start.min(len))..rope.char_to_utf16_cu(range.end.min(len))
}

impl EditorView {
    fn text_input_blocked(&self) -> bool {
        self.worklist_document && !self.worklist_raw
    }

    fn finish_text_edit(&mut self, cx: &mut Context<Self>) {
        self.recompute_highlights();
        self.emit_buffer_changed(cx);
        if self.find_is_open() {
            self.rescan_find();
        }
        self.prose.goal_x = None;
        cx.notify();
    }
}

impl EntityInputHandler for EditorView {
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text_input_blocked() {
            return;
        }
        self.cursor_blink.reset(cx, Self::blink_tick);
        let marked = self.prose.marked.take();
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let target = range
            .map(|r| utf16_to_chars(buffer.rope(), &r))
            .or(marked.clone());
        if let Some(target) = target {
            // Commit (or replace an explicit range): one undo step for the lot.
            buffer.replace_range(target, text);
            buffer.set_undo_group(false);
            self.finish_text_edit(cx);
            return;
        }
        if text.is_empty() {
            return;
        }
        if xenon_settings::vim_mode(cx) && !self.prose_active() && self.handle_vim_char(text, cx) {
            cx.notify();
            return;
        }
        if self.prose_active() {
            self.prose_type(text, cx);
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if xenon_settings::vim_mode(cx) {
            self.vim.note_insert_text(text);
        }
        buffer.apply(EditCommand::Insert(text.to_string()));
        self.finish_text_edit(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text_input_blocked() {
            return;
        }
        self.cursor_blink.reset(cx, Self::blink_tick);
        self.end_typing();
        let marked = self.prose.marked.take();
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let target = range
            .map(|r| utf16_to_chars(buffer.rope(), &r))
            .or(marked.clone())
            .or_else(|| buffer.selection_range())
            .unwrap_or_else(|| buffer.cursor()..buffer.cursor());
        if marked.is_none() {
            buffer.set_undo_group(true);
        }
        let start = target.start;
        buffer.replace_range(target, new_text);
        let len = new_text.chars().count();
        if len == 0 {
            buffer.set_undo_group(false);
        } else {
            self.prose.marked = Some(start..start + len);
            if let Some(sel) = new_selected_range {
                let local = new_text.encode_utf16().take(sel.end).collect::<Vec<u16>>();
                let chars = String::from_utf16_lossy(&local).chars().count();
                buffer.set_cursor_raw(start + chars);
            }
        }
        self.finish_text_edit(cx);
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let rope = buffer.rope();
        let (range, reversed) = match buffer.selection_range() {
            Some(r) => {
                let reversed = buffer.cursor() == r.start;
                (r, reversed)
            }
            None => (buffer.cursor()..buffer.cursor(), false),
        };
        Some(UTF16Selection {
            range: chars_to_utf16(rope, &range),
            reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        self.prose
            .marked
            .as_ref()
            .map(|r| chars_to_utf16(buffer.rope(), r))
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let rope = buffer.rope();
        let chars = utf16_to_chars(rope, &range);
        *adjusted = Some(chars_to_utf16(rope, &chars));
        Some(rope.slice(chars).to_string())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.prose.marked.take().is_some() {
            if let Content::Text(buffer) = &mut self.content {
                buffer.set_undo_group(false);
            }
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if !self.prose_active() {
            return None;
        }
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let chars = utf16_to_chars(buffer.rope(), &range_utf16);
        self.prose_caret_bounds(chars.start)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
