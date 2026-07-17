//! EntityInputHandler for macOS typed text into the editor / find bar.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::{Content, EditorView};
use crate::edit::EditCommand;

impl EntityInputHandler for EditorView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            return;
        }
        if self.find_bar_focused(window) {
            self.append_find_query(text, cx);
            return;
        }
        if xero_settings::vim_mode(cx) && self.handle_vim_char(text, cx) {
            cx.notify();
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if xero_settings::vim_mode(cx) {
            self.vim.note_insert_text(text);
        }
        buffer.apply(EditCommand::Insert(text.to_string()));
        self.recompute_highlights();
        if self.find_is_open() {
            self.rescan_find();
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if new_text.is_empty() {
            return;
        }
        if self.find_bar_focused(window) {
            self.append_find_query(new_text, cx);
            return;
        }
        if xero_settings::vim_mode(cx) && self.handle_vim_char(new_text, cx) {
            cx.notify();
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if xero_settings::vim_mode(cx) {
            self.vim.note_insert_text(new_text);
        }
        buffer.apply(EditCommand::Insert(new_text.to_string()));
        self.recompute_highlights();
        if self.find_is_open() {
            self.rescan_find();
        }
        cx.notify();
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
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
