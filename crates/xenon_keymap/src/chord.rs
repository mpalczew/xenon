//! One keystroke, parsed from gpui's `cmd-shift-p` form or from the glyph
//! strings the UI hand-writes (`⌘⇧P`), and formatted for a platform.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Linux,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Linux
        }
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

    pub fn format(&self, platform: Platform) -> String {
        match platform {
            Platform::Mac => self.format_mac(),
            Platform::Linux => self.format_linux(),
        }
    }

    fn format_mac(&self) -> String {
        let mut out = String::new();
        for (on, glyph) in [
            (self.ctrl, '⌃'),
            (self.alt, '⌥'),
            (self.shift, '⇧'),
            (self.cmd, '⌘'),
        ] {
            if on {
                out.push(glyph);
            }
        }
        out + &key_text(&self.key, Platform::Mac)
    }

    fn format_linux(&self) -> String {
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
        parts.push(key_text(&self.key, Platform::Linux));
        parts.join("+")
    }

    /// The same chord with ⌘ replaced by the platform's secondary modifier.
    pub fn for_platform(mut self, platform: Platform) -> Self {
        if platform == Platform::Linux && self.cmd {
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

/// Format a (possibly multi-keystroke) gpui binding for the platform.
pub fn format_sequence(keystrokes: &str, platform: Platform) -> String {
    keystrokes
        .split_whitespace()
        .map(|part| match Chord::parse(part) {
            Some(chord) => chord.format(platform),
            None => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn key_text(key: &str, platform: Platform) -> String {
    let named = match (key, platform) {
        ("enter", Platform::Mac) => "↩",
        ("enter", Platform::Linux) => "Enter",
        ("tab", Platform::Mac) => "⇥",
        ("tab", Platform::Linux) => "Tab",
        ("backspace", Platform::Mac) => "⌫",
        ("backspace", Platform::Linux) => "Backspace",
        ("escape", Platform::Mac) => "⎋",
        ("escape", Platform::Linux) => "Esc",
        ("space", _) => "Space",
        ("up", _) => "↑",
        ("down", _) => "↓",
        ("left", _) => "←",
        ("right", _) => "→",
        ("pageup", _) => "PgUp",
        ("pagedown", _) => "PgDn",
        _ => return key.to_uppercase(),
    };
    named.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gpui_and_glyph_forms_to_one_identity() {
        let a = Chord::parse("cmd-alt-shift-w").unwrap();
        let b = Chord::from_glyphs("⌘⌥⇧W").unwrap();
        assert_eq!(a.canonical(), b.canonical());
        assert_eq!(Chord::parse("cmd--").unwrap().canonical(), "cmd--");
        assert_eq!(
            Chord::from_glyphs("⌃⇥").unwrap().canonical(),
            Chord::parse("ctrl-tab").unwrap().canonical()
        );
        assert_eq!(
            Chord::parse("f12").unwrap(),
            Chord::from_glyphs("F12").unwrap()
        );
    }

    #[test]
    fn linux_formatting_uses_names() {
        let chord = Chord::parse("cmd-shift-p")
            .unwrap()
            .for_platform(Platform::Linux);
        assert_eq!(chord.format(Platform::Linux), "Ctrl+Shift+P");
        let enter = Chord::from_glyphs("⌘↩")
            .unwrap()
            .for_platform(Platform::Linux);
        assert_eq!(enter.format(Platform::Linux), "Ctrl+Enter");
    }

    #[test]
    fn mac_formatting_uses_glyphs_in_standard_order() {
        let chord = Chord::parse("cmd-alt-shift-w").unwrap();
        assert_eq!(chord.format(Platform::Mac), "⌥⇧⌘W");
    }

    #[test]
    fn prose_is_not_a_chord() {
        assert!(Chord::from_glyphs("⌘↩ / ⌃↩").is_none());
        assert!(Chord::from_glyphs("⌘-click").is_none());
    }

    #[test]
    fn formats_sequences() {
        assert_eq!(
            format_sequence("ctrl-k ctrl-t", Platform::Linux),
            "Ctrl+K Ctrl+T"
        );
    }

    #[test]
    fn rejects_empty_key() {
        assert!(Chord::parse("").is_none());
        assert!(Chord::from_glyphs("⌘").is_none());
    }
}
