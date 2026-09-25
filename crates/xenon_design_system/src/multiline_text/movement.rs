use super::{MultilineText, char_byte};

impl MultilineText {
    pub fn up(&mut self, extend: bool) {
        let chars: Vec<char> = self.text.chars().collect();
        let start = chars[..self.caret]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1);
        let column = self.caret - start;
        if start == 0 {
            self.caret = 0;
        } else {
            let previous_end = start - 1;
            let previous_start = chars[..previous_end]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |i| i + 1);
            self.caret = (previous_start + column).min(previous_end);
        }
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn down(&mut self, extend: bool) {
        let chars: Vec<char> = self.text.chars().collect();
        let start = chars[..self.caret]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1);
        let column = self.caret - start;
        let Some(end) = chars[self.caret..]
            .iter()
            .position(|c| *c == '\n')
            .map(|i| self.caret + i)
        else {
            self.caret = chars.len();
            if !extend {
                self.anchor = self.caret;
            }
            return;
        };
        let next_start = end + 1;
        let next_end = chars[next_start..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(chars.len(), |i| next_start + i);
        self.caret = (next_start + column).min(next_end);
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn backspace(&mut self) {
        if self.anchor == self.caret && self.caret > 0 {
            self.anchor = self.caret - 1;
        }
        self.replace(None, "", false);
    }
    pub fn delete(&mut self) {
        if self.anchor == self.caret {
            self.anchor = (self.caret + 1).min(self.text.chars().count());
        }
        self.replace(None, "", false);
    }
    pub fn split_at_caret(&self) -> (String, String, String) {
        let range = self.selection();
        let start = char_byte(&self.text, range.start);
        let end = char_byte(&self.text, range.end);
        (
            self.text[..start].into(),
            self.text[start..end].into(),
            self.text[end..].into(),
        )
    }
}
