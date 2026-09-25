use super::{MultilineText, char_byte, char_utf16, utf16_chars};
use std::ops::Range;

impl MultilineText {
    pub fn replace(&mut self, range: Option<Range<usize>>, value: &str, mark: bool) {
        let selected = range
            .map(|range| utf16_chars(&self.text, range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        let start = char_byte(&self.text, selected.start);
        let end = char_byte(&self.text, selected.end);
        self.text.replace_range(start..end, value);
        self.caret = selected.start + value.chars().count();
        self.anchor = self.caret;
        self.marked = (mark && !value.is_empty()).then_some(selected.start..self.caret);
    }
    pub(crate) fn select_marked_range(&mut self, selected_utf16: Range<usize>) {
        let Some(marked) = self.marked.clone() else {
            return;
        };
        let marked_text =
            &self.text[char_byte(&self.text, marked.start)..char_byte(&self.text, marked.end)];
        let relative = utf16_chars(marked_text, selected_utf16);
        self.anchor = marked.start + relative.start;
        self.caret = marked.start + relative.end;
    }
    pub fn text_for_utf16_range(&self, range: Range<usize>) -> String {
        let range = utf16_chars(&self.text, range);
        self.text[char_byte(&self.text, range.start)..char_byte(&self.text, range.end)].to_string()
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.chars().count();
    }
    pub fn selected_utf16(&self) -> Range<usize> {
        let range = self.selection();
        char_utf16(&self.text, range.start)..char_utf16(&self.text, range.end)
    }
    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|r| char_utf16(&self.text, r.start)..char_utf16(&self.text, r.end))
    }
    pub fn unmark(&mut self) {
        self.marked = None;
    }
}
