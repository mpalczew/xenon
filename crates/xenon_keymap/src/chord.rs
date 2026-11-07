//! One keystroke, parsed from gpui's `cmd-shift-p` form or from the glyph
//! strings the UI hand-writes (`⌘⇧P`), and formatted for a platform.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
}

impl Platform {
    pub const fn current() -> Self {
        Self::Linux
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    ctrl: bool,
    alt: bool,
    shift: bool,
    cmd: bool,
    key: String,
}

impl Chord {
    /// gpui form: `cmd-alt-shift-w`, `cmd--`, `f12`. One keystroke only.
    pub fn parse(text: &str) -> Option<Self> {
        let mut chord = Self::default();
        let mut rest = text;
        while let Some((head, tail)) = rest.split_once('-') {
            if tail.is_empty() || !chord.set_modifier(head) {
                break;
            }
            rest = tail;
        }
        chord.key = rest.to_owned();
        (!chord.key.is_empty()).then_some(chord)
    }

    fn set_modifier(&mut self, name: &str) -> bool {
        match name {
            "ctrl" => self.ctrl = true,
            "alt" => self.alt = true,
            "shift" => self.shift = true,
            "cmd" | "super" | "win" => self.cmd = true,
            _ => return false,
        }
        true
    }

    /// Glyph form: `⌘⇧P`, `⌃⇥`, `⌥↩`, `F12`.
    pub fn from_glyphs(text: &str) -> Option<Self> {
        let mut chord = Self::default();
        let mut key = String::new();
        for ch in text.chars() {
            match ch {
                '⌘' => chord.cmd = true,
                '⌃' => chord.ctrl = true,
                '⌥' => chord.alt = true,
                '⇧' => chord.shift = true,
                other => key.push(other),
            }
        }
        chord.key = glyph_key(&key)?;
        Some(chord)
    }

    /// Stable identity for lookups, independent of modifier order or glyphs.
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.ctrl, "ctrl-"),
            (self.alt, "alt-"),
            (self.shift, "shift-"),
            (self.cmd, "cmd-"),
        ] {
            if on {
                out.push_str(name);
            }
        }
        out + &self.key
    }

    /// `Ctrl+Shift+P`, `Alt+Enter`, `F12`.
    pub fn format(&self) -> String {
        let mut parts = Vec::new();
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.cmd, "Super"),
        ] {
            if on {
                parts.push(name.to_owned());
            }
        }
        parts.push(key_text(&self.key));
        parts.join("+")
    }

    /// The same chord with ⌘ replaced by Ctrl, the secondary modifier here.
    pub fn for_linux(mut self) -> Self {
        if self.cmd {
            self.cmd = false;
            self.ctrl = true;
        }
        self
    }
}

fn glyph_key(text: &str) -> Option<String> {
    let key = match text {
        "" => return None,
        "↩" => "enter",
        "⇥" => "tab",
        "⌫" => "backspace",
        "⎋" => "escape",
        "↑" => "up",
        "↓" => "down",
        "←" => "left",
        "→" => "right",
        other if is_single_key(other) => return Some(other.to_lowercase()),
        _ => return None,
    };
    Some(key.to_owned())
}

/// One character, or a function key like `F12`. Anything longer is prose.
fn is_single_key(text: &str) -> bool {
    text.chars().count() == 1
        || (text.len() > 1
            && text.starts_with('F')
            && text[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Format a (possibly multi-keystroke) gpui binding.
pub fn format_sequence(keystrokes: &str) -> String {
    keystrokes
        .split_whitespace()
        .map(|part| match Chord::parse(part) {
            Some(chord) => chord.format(),
            None => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn key_text(key: &str) -> String {
    let named = match key {
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "escape" => "Esc",
        "space" => "Space",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "pageup" => "PgUp",
        "pagedown" => "PgDn",
        _ => return key.to_uppercase(),
    };
    named.to_owned()
}
