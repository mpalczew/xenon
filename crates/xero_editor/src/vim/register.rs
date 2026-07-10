//! Named and unnamed registers for yank/delete/paste.

use std::collections::HashMap;

#[derive(Default)]
pub struct Registers {
    unnamed: String,
    named: HashMap<char, String>,
    /// Last explicit register prefix (`"a`), if any.
    pending: Option<char>,
}

impl Registers {
    pub fn set_pending(&mut self, name: char) {
        self.pending = Some(name);
    }

    pub fn take_pending(&mut self) -> Option<char> {
        self.pending.take()
    }

    pub fn pending(&self) -> Option<char> {
        self.pending
    }

    /// Store text into the pending register (or unnamed). Uppercase appends.
    pub fn yank(&mut self, text: &str) {
        let name = self.pending.take().unwrap_or('"');
        self.write(name, text, false);
        if name != '"' {
            self.unnamed = text.to_string();
        }
    }

    /// Delete also updates the unnamed register (unless black-hole `_`).
    pub fn delete(&mut self, text: &str) {
        let name = self.pending.take().unwrap_or('"');
        if name == '_' {
            return;
        }
        self.write(name, text, false);
        if name != '"' {
            self.unnamed = text.to_string();
        }
    }

    pub fn paste_text(&mut self) -> String {
        let name = self.pending.take().unwrap_or('"');
        self.read(name)
    }

    fn write(&mut self, name: char, text: &str, _force: bool) {
        match name {
            '"' => self.unnamed = text.to_string(),
            'a'..='z' => {
                self.named.insert(name, text.to_string());
            }
            'A'..='Z' => {
                let lower = name.to_ascii_lowercase();
                let entry = self.named.entry(lower).or_default();
                entry.push_str(text);
                self.unnamed = entry.clone();
            }
            '+' | '*' => {
                // System clipboard is handled by the view; still keep unnamed in sync.
                self.unnamed = text.to_string();
            }
            '_' => {}
            _ => {
                self.unnamed = text.to_string();
            }
        }
    }

    fn read(&self, name: char) -> String {
        match name {
            '"' => self.unnamed.clone(),
            'a'..='z' => self.named.get(&name).cloned().unwrap_or_default(),
            'A'..='Z' => self
                .named
                .get(&name.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default(),
            '+' | '*' => self.unnamed.clone(),
            _ => self.unnamed.clone(),
        }
    }

    pub fn set_unnamed(&mut self, text: &str) {
        self.unnamed = text.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_yank_and_paste() {
        let mut regs = Registers::default();
        regs.set_pending('a');
        regs.yank("hello");
        regs.set_pending('a');
        assert_eq!(regs.paste_text(), "hello");
        // Unnamed still holds the last yank when no pending register.
        assert_eq!(regs.paste_text(), "hello");
    }

    #[test]
    fn append_register() {
        let mut regs = Registers::default();
        regs.set_pending('a');
        regs.yank("hi");
        regs.set_pending('A');
        regs.yank("!");
        regs.set_pending('a');
        assert_eq!(regs.paste_text(), "hi!");
    }
}
