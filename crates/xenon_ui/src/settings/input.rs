//! Text input handler for filterable settings dropdowns.

use std::ops::Range;

use gpui::{Context, EntityInputHandler, UTF16Selection, Window};

use crate::entity_input_noop_geometry;

use super::{SettingsView, is_filterable};

impl EntityInputHandler for SettingsView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.open
            && is_filterable(id)
        {
            self.filter.push_str(text);
            self.highlight = 0;
            self.caret_on = true;
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(None, new_text, window, cx);
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        Some(self.filter.clone())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let len = self.filter.encode_utf16().count();
        Some(UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    entity_input_noop_geometry!();
}
