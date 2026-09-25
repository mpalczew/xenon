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
    pub fn selected_text(&self) -> &str {
        let range = self.selection();
        &self.text[char_byte(&self.text, range.start)..char_byte(&self.text, range.end)]
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_in_the_middle_and_replaces_ime_marked_text() {
        let mut edit = MultilineText::new("a🦀c");
        edit.left(false);
        edit.replace(None, "b", false);
        assert_eq!(edit.text(), "a🦀bc");
        edit.replace(None, "に", true);
        edit.replace(None, "日本", true);
        edit.replace(None, "日本語", false);
        assert_eq!(edit.text(), "a🦀b日本語c");
    }
    #[test]
    fn multiline_arrows_keep_column() {
        let mut edit = MultilineText::new("one\ntwo\nlonger");
        edit.home(false);
        edit.right(false);
        edit.up(false);
        assert_eq!(edit.selected_utf16(), 5..5);
        edit.down(false);
        assert_eq!(edit.selected_utf16(), 9..9);
    }
    #[test]
    fn marked_selection_uses_utf16_offsets() {
        let mut edit = MultilineText::new("a");
        edit.replace(None, "🦀b", true);
        edit.select_marked_range(2..2);
        assert_eq!(edit.selected_utf16(), 3..3);
        assert_eq!(edit.marked_utf16(), Some(1..4));
    }
}
