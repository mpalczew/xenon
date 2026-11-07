//! Editable multiline text for text fields.

use std::ops::Range;

#[derive(Default)]
pub struct MultilineText {
    pub(super) text: String,
    pub(super) caret: usize,
    pub(super) anchor: usize,
    pub(super) marked: Option<Range<usize>>,
}

mod input;
mod movement;
mod navigation;

impl MultilineText {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        Self {
            text,
            caret,
            anchor: caret,
            marked: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub(crate) fn is_caret_at_start(&self) -> bool {
        let selection = self.selection();
        selection.is_empty() && selection.start == 0
    }

    /// Drop the selection and everything after it. The caret stays on the kept prefix.
    pub(crate) fn split_off_suffix(&mut self) -> String {
        let range = self.selection();
        let end = char_byte(&self.text, range.end);
        let suffix = self.text[end..].to_owned();
        self.text.truncate(char_byte(&self.text, range.start));
        let caret = self.text.chars().count();
        self.caret = caret;
        self.anchor = caret;
        self.marked = None;
        suffix
    }

    pub(crate) fn points(&self) -> (String, usize, usize) {
        (self.text.clone(), self.caret, self.anchor)
    }

    pub(crate) fn restore(&mut self, text: String, caret: usize, anchor: usize) {
        let len = text.chars().count();
        self.text = text;
        self.caret = caret.min(len);
        self.anchor = anchor.min(len);
        self.marked = None;
    }

    pub(crate) fn set_caret(&mut self, index: usize) {
        let index = index.min(self.text.chars().count());
        self.caret = index;
        self.anchor = index;
        self.marked = None;
    }
    pub fn selected_text(&self) -> &str {
        let range = self.selection();
        &self.text[char_byte(&self.text, range.start)..char_byte(&self.text, range.end)]
    }
    pub(crate) fn selected_byte_range(&self) -> Range<usize> {
        let range = self.selection();
        char_byte(&self.text, range.start)..char_byte(&self.text, range.end)
    }
}

pub(super) fn char_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}

pub(super) fn char_utf16(text: &str, index: usize) -> usize {
    text.chars().take(index).map(char::len_utf16).sum()
}

pub(super) fn utf16_chars(text: &str, range: Range<usize>) -> Range<usize> {
    fn at(text: &str, offset: usize) -> usize {
        let mut units = 0;
        for (index, ch) in text.chars().enumerate() {
            if units >= offset {
                return index;
            }
            units += ch.len_utf16();
        }
        text.chars().count()
    }
    at(text, range.start)..at(text, range.end)
}
