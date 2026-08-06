//! Named and unnamed registers for yank/delete/paste.
//!
//! Each entry tracks whether content is **linewise** (from `dd`/`yy`/`V`, etc.)
//! so paste can put whole lines above/below the cursor like classic vim.

use std::collections::HashMap;

/// Text taken from a register for paste.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterContent {
    pub text: String,
    pub linewise: bool,
    /// From visual-block yank/delete; paste inserts column-wise.
    pub blockwise: bool,
}

#[derive(Default)]
pub struct Registers {
    unnamed: RegisterContent,
    named: HashMap<char, RegisterContent>,
    /// Last explicit register prefix (`"a`), if any.
    pending: Option<char>,
}

impl Registers {
    pub(crate) fn set_pending(&mut self, name: char) {
        self.pending = Some(name);
    }

    pub(crate) fn take_pending(&mut self) -> Option<char> {
        self.pending.take()
    }

    pub(crate) fn pending(&self) -> Option<char> {
        self.pending
    }

    /// Store text into the pending register (or unnamed). Uppercase appends.
    pub(crate) fn yank(&mut self, text: &str, linewise: bool) {
        self.yank_ex(text, linewise, false);
    }

    pub(crate) fn yank_block(&mut self, text: &str) {
        self.yank_ex(text, false, true);
    }

    fn yank_ex(&mut self, text: &str, linewise: bool, blockwise: bool) {
        let name = self.pending.take().unwrap_or('"');
        self.write(name, text, linewise, blockwise);
        if name != '"' {
            self.unnamed = RegisterContent {
                text: text.to_string(),
                linewise,
                blockwise,
            };
        }
    }

    /// Delete also updates the unnamed register (unless black-hole `_`).
    pub(crate) fn delete(&mut self, text: &str, linewise: bool) {
        self.delete_ex(text, linewise, false);
    }

    pub(crate) fn delete_block(&mut self, text: &str) {
        self.delete_ex(text, false, true);
    }

    fn delete_ex(&mut self, text: &str, linewise: bool, blockwise: bool) {
        let name = self.pending.take().unwrap_or('"');
        if name == '_' {
            return;
        }
        self.write(name, text, linewise, blockwise);
        if name != '"' {
            self.unnamed = RegisterContent {
                text: text.to_string(),
                linewise,
                blockwise,
            };
        }
    }

    pub(crate) fn paste(&mut self) -> RegisterContent {
        let name = self.pending.take().unwrap_or('"');
        self.read(name)
    }

    /// Characterwise content into the unnamed register (system clipboard).
    pub(crate) fn set_unnamed(&mut self, text: &str) {
        self.unnamed = RegisterContent {
            text: text.to_string(),
            linewise: false,
            blockwise: false,
        };
    }

    fn write(&mut self, name: char, text: &str, linewise: bool, blockwise: bool) {
        let content = RegisterContent {
            text: text.to_string(),
            linewise,
            blockwise,
        };
        match name {
            '"' => self.unnamed = content,
            'a'..='z' => {
                self.named.insert(name, content);
            }
            'A'..='Z' => {
                let lower = name.to_ascii_lowercase();
                let entry = self.named.entry(lower).or_default();
                entry.text.push_str(text);
                // Vim: result is linewise if either side is linewise.
                entry.linewise = entry.linewise || linewise;
                entry.blockwise = entry.blockwise || blockwise;
                self.unnamed = entry.clone();
            }
            '+' | '*' => {
                // System clipboard is handled by the view; still keep unnamed in sync.
                self.unnamed = content;
            }
            '_' => {}
            _ => {
                self.unnamed = content;
            }
        }
    }

    fn read(&self, name: char) -> RegisterContent {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_yank_and_paste() {
        let mut regs = Registers::default();
        regs.set_pending('a');
        regs.yank("hello", false);
        regs.set_pending('a');
        let got = regs.paste();
        assert_eq!(got.text, "hello");
        assert!(!got.linewise);
        // Unnamed still holds the last yank when no pending register.
        assert_eq!(regs.paste().text, "hello");
    }

    #[test]
    fn append_register() {
        let mut regs = Registers::default();
        regs.set_pending('a');
        regs.yank("hi", false);
        regs.set_pending('A');
        regs.yank("!", false);
        regs.set_pending('a');
        assert_eq!(regs.paste().text, "hi!");
    }

    #[test]
    fn linewise_flag_survives_paste() {
        let mut regs = Registers::default();
        regs.yank("foo\n", true);
        let got = regs.paste();
        assert_eq!(got.text, "foo\n");
        assert!(got.linewise);
    }

    #[test]
    fn append_promotes_to_linewise() {
        let mut regs = Registers::default();
        regs.set_pending('a');
        regs.yank("x", false);
        regs.set_pending('A');
        regs.yank("y\n", true);
        regs.set_pending('a');
        let got = regs.paste();
        assert_eq!(got.text, "xy\n");
        assert!(got.linewise);
    }
}
