use super::MultilineText;

impl MultilineText {
    pub(crate) fn place_caret(&mut self, index: usize, extend: bool) {
        self.caret = index.min(self.text.chars().count());
        if !extend {
            self.anchor = self.caret;
        }
        self.marked = None;
    }
    pub fn left(&mut self, extend: bool) {
        if !extend && self.anchor != self.caret {
            self.caret = self.selection().start;
        } else {
            self.caret = self.caret.saturating_sub(1);
        }
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn right(&mut self, extend: bool) {
        if !extend && self.anchor != self.caret {
            self.caret = self.selection().end;
        } else {
            self.caret = (self.caret + 1).min(self.text.chars().count());
        }
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn home(&mut self, extend: bool) {
        let before: Vec<char> = self.text.chars().take(self.caret).collect();
        self.caret = before.iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1);
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn end(&mut self, extend: bool) {
        let chars: Vec<char> = self.text.chars().collect();
        self.caret = chars[self.caret..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(chars.len(), |i| self.caret + i);
        if !extend {
            self.anchor = self.caret;
        }
    }
}
