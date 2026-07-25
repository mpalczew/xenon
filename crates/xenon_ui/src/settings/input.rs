//! Text input handler for filterable settings dropdowns + remote line fields.

use std::ops::Range;

use gpui::{Context, EntityInputHandler, UTF16Selection, Window};

use crate::entity_input_noop_geometry;

use super::{SettingsView, is_filterable};

impl EntityInputHandler for SettingsView {
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(re) = self.remote_edit.as_mut() {
            re.edit.replace_utf16_range(range, text);
            self.caret_on = true;
            cx.notify();
            return;
        }
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
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, new_text, window, cx);
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(re) = &self.remote_edit {
            return Some(re.edit.text().to_string());
        }
        Some(self.filter.clone())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if let Some(re) = &self.remote_edit {
            let range = re.edit.utf16_selection();
            return Some(UTF16Selection {
                range,
                reversed: re.edit.caret() < re.edit.selection().start
                    || (re.edit.has_selection() && re.edit.caret() == re.edit.selection().start),
            });
        }
        let len = self.filter.encode_utf16().count();
        Some(UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    entity_input_noop_geometry!();
}
