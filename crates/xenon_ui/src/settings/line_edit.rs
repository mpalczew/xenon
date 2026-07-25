//! Single-line text edit state: caret, selection, arrows, insert/delete.
//! Not a native widget — but behaves like a normal field for keyboard entry.

use std::ops::Range;

/// Editable single-line buffer with UTF-8-safe caret + selection.
#[derive(Clone, Debug)]
pub(super) struct LineEdit {
    text: String,
    /// Caret as char index `0..=char_count` (selection collapsed when start==end).
    caret: usize,
    /// Selection anchor char index; range is min(anchor,caret)..max(anchor,caret).
    anchor: usize,
}

impl LineEdit {
    pub(super) fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.chars().count();
        Self {
            text,
            caret: end,
            anchor: end,
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn into_text(self) -> String {
        self.text
    }

    /// Char count.
    pub(super) fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    pub(super) fn caret(&self) -> usize {
        self.caret
    }

    /// Selected char range (may be empty).
    pub(super) fn selection(&self) -> Range<usize> {
        let a = self.anchor.min(self.caret);
        let b = self.anchor.max(self.caret);
        a..b
    }

    pub(super) fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }

    /// Text before caret / selection start, selected mid, after selection end.
    pub(super) fn split_for_paint(&self) -> (String, String, String) {
        let sel = self.selection();
        let chars: Vec<char> = self.text.chars().collect();
        let before: String = chars[..sel.start].iter().collect();
        let mid: String = chars[sel.start..sel.end].iter().collect();
        let after: String = chars[sel.end..].iter().collect();
        (before, mid, after)
    }

    pub(super) fn move_left(&mut self, extend: bool) {
        if !extend && self.has_selection() {
            let start = self.selection().start;
            self.caret = start;
            self.anchor = start;
            return;
        }
        if self.caret > 0 {
            self.caret -= 1;
        }
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub(super) fn move_right(&mut self, extend: bool) {
        if !extend && self.has_selection() {
            let end = self.selection().end;
            self.caret = end;
            self.anchor = end;
            return;
        }
        if self.caret < self.len_chars() {
            self.caret += 1;
        }
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub(super) fn home(&mut self, extend: bool) {
        self.caret = 0;
        if !extend {
            self.anchor = 0;
        }
    }

    pub(super) fn end(&mut self, extend: bool) {
        self.caret = self.len_chars();
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub(super) fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len_chars();
    }

    pub(super) fn backspace(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.caret == 0 {
            return;
        }
        self.caret -= 1;
        self.anchor = self.caret;
        self.text = {
            let chars: Vec<char> = self.text.chars().collect();
            chars
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.caret)
                .map(|(_, c)| *c)
                .collect()
        };
    }

    pub(super) fn delete_forward(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.caret >= self.len_chars() {
            return;
        }
        self.text = {
            let chars: Vec<char> = self.text.chars().collect();
            chars
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.caret)
                .map(|(_, c)| *c)
                .collect()
        };
        self.anchor = self.caret;
    }

    /// Insert `s` at caret (replacing selection). Strips newlines.
    pub(super) fn insert(&mut self, s: &str) {
        let s: String = s.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        if s.is_empty() && !self.has_selection() {
            return;
        }
        self.delete_selection();
        let chars: Vec<char> = self.text.chars().collect();
        let mut out = String::with_capacity(self.text.len() + s.len());
        for (i, c) in chars.iter().enumerate() {
            if i == self.caret {
                out.push_str(&s);
            }
            out.push(*c);
        }
        if self.caret >= chars.len() {
            out.push_str(&s);
        }
        let added = s.chars().count();
        self.text = out;
        self.caret += added;
        self.anchor = self.caret;
    }

    fn delete_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        let sel = self.selection();
        let chars: Vec<char> = self.text.chars().collect();
        self.text = chars
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < sel.start || *i >= sel.end)
            .map(|(_, c)| *c)
            .collect();
        self.caret = sel.start;
        self.anchor = sel.start;
    }

    /// UTF-16 selection range for the platform input system.
    pub(super) fn utf16_selection(&self) -> Range<usize> {
        let sel = self.selection();
        let mut start = 0;
        let mut end = 0;
        for (i, c) in self.text.chars().enumerate() {
            let w = c.len_utf16();
            if i < sel.start {
                start += w;
            }
            if i < sel.end {
                end += w;
            }
        }
        start..end
    }

    /// Apply IME/platform replace. `range` is UTF-16; None → insert at caret.
    pub(super) fn replace_utf16_range(&mut self, range: Option<Range<usize>>, text: &str) {
        if let Some(r) = range {
            // Map UTF-16 range → char indices.
            let (start, end) = utf16_range_to_chars(&self.text, r);
            self.anchor = start;
            self.caret = end;
            self.delete_selection();
            self.insert(text);
        } else {
            self.insert(text);
        }
    }
}

fn utf16_range_to_chars(text: &str, range: Range<usize>) -> (usize, usize) {
    let mut u16_i = 0;
    let mut start_c = None;
    let mut end_c = None;
    for (ci, c) in text.chars().enumerate() {
        if start_c.is_none() && u16_i >= range.start {
            start_c = Some(ci);
        }
        if end_c.is_none() && u16_i >= range.end {
            end_c = Some(ci);
        }
        u16_i += c.len_utf16();
    }
    let n = text.chars().count();
    if start_c.is_none() && u16_i >= range.start {
        start_c = Some(n);
    }
    if end_c.is_none() && u16_i >= range.end {
        end_c = Some(n);
    }
    (start_c.unwrap_or(n).min(n), end_c.unwrap_or(n).min(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_middle() {
        let mut e = LineEdit::new("hello");
        e.home(false);
        e.move_right(false);
        e.move_right(false);
        e.insert("XX");
        assert_eq!(e.text(), "heXXllo");
        assert_eq!(e.caret(), 4);
    }

    #[test]
    fn backspace_middle() {
        let mut e = LineEdit::new("abcd");
        e.caret = 2;
        e.anchor = 2;
        e.backspace();
        assert_eq!(e.text(), "acd");
        assert_eq!(e.caret(), 1);
    }

    #[test]
    fn select_all_replace() {
        let mut e = LineEdit::new("old");
        e.select_all();
        e.insert("new");
        assert_eq!(e.text(), "new");
    }

    #[test]
    fn arrows_and_delete() {
        let mut e = LineEdit::new("ab");
        e.home(false);
        e.delete_forward();
        assert_eq!(e.text(), "b");
        e.end(false);
        e.insert("c");
        assert_eq!(e.text(), "bc");
    }
}
