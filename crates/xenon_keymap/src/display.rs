//! Shortcut text for UI chrome. Call sites hand in the macOS glyph string they
//! have always written (`⌘⇧P`); this returns what the current platform and the
//! user's keymap actually bind.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::{Chord, Platform};

fn overrides() -> &'static RwLock<HashMap<String, String>> {
    static OVERRIDES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    OVERRIDES.get_or_init(Default::default)
}

/// Replace the user-keymap overrides (see [`crate::display_overrides`]).
pub fn set_display_overrides(map: HashMap<String, String>) {
    if let Ok(mut current) = overrides().write() {
        *current = map;
    }
}

pub fn display_keys(glyphs: &str) -> String {
    display_for(glyphs, Platform::current())
}

/// Translate every shortcut inside a sentence (`Close Tab · ⌘W`). Words that
/// merely follow a modifier (`⌘-click`) keep their text; the modifier is named.
pub fn display_text(text: &str) -> String {
    translate(text, Platform::current())
}

fn translate(text: &str, platform: Platform) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_modifier(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_modifier(chars[i]) {
            i += 1;
        }
        let key_end = i + key_token_len(&chars[i..]);
        let token: String = chars[start..key_end].iter().collect();
        out.push_str(&display_token(&token, platform));
        i = key_end;
    }
    out
}

fn is_modifier(c: char) -> bool {
    matches!(c, '⌘' | '⌃' | '⌥' | '⇧')
}

/// Length of the key that follows modifiers, or 0 when prose follows.
fn key_token_len(rest: &[char]) -> usize {
    let Some(&first) = rest.first() else {
        return 0;
    };
    if "↩↑↓←→⇥⌫⎋".contains(first) {
        return 1;
    }
    if first.is_ascii_alphabetic() {
        let word: String = rest
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let is_function = word.starts_with('F')
            && word.len() > 1
            && word[1..].chars().all(|c| c.is_ascii_digit());
        return match word.as_str() {
            "return" | "esc" => word.len(),
            _ if word.len() == 1 || is_function => word.len(),
            _ => 0,
        };
    }
    let punctuation = ",./[]\\`=+;?";
    let hyphen_is_key = first == '-' && !rest.get(1).is_some_and(|c| c.is_alphanumeric());
    usize::from(first.is_ascii_digit() || punctuation.contains(first) || hyphen_is_key)
}

fn display_token(token: &str, platform: Platform) -> String {
    let normalized = token.replace("return", "↩").replace("esc", "⎋");
    match Chord::from_glyphs(&normalized) {
        Some(_) => {
            let shown = display_for(&normalized, platform);
            // macOS keeps the author's words (`⌘return`) unless the user rebound it.
            if platform == Platform::Mac && shown == normalized {
                token.to_owned()
            } else {
                shown
            }
        }
        None if platform == Platform::Linux => modifier_names(token),
        None => token.to_owned(),
    }
}

/// A modifier-only token such as `⌘` becomes `Ctrl` (or `Alt`, `Shift`).
fn modifier_names(token: &str) -> String {
    let names: Vec<&str> = token
        .chars()
        .map(|c| match c {
            '⌃' | '⌘' => "Ctrl",
            '⌥' => "Alt",
            _ => "Shift",
        })
        .collect();
    names.join("+")
}

fn display_for(glyphs: &str, platform: Platform) -> String {
    let Some(chord) = Chord::from_glyphs(glyphs) else {
        return glyphs.to_owned();
    };
    let overridden = overrides()
        .read()
        .ok()
        .and_then(|map| map.get(&chord.canonical()).cloned());
    match (overridden, platform) {
        (Some(text), _) => text,
        (None, Platform::Mac) => glyphs.to_owned(),
        (None, Platform::Linux) => chord.for_platform(platform).format(platform),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_text_is_untouched() {
        assert_eq!(display_for("⌘⌥⇧W", Platform::Mac), "⌘⌥⇧W");
    }

    #[test]
    fn linux_text_names_keys() {
        assert_eq!(display_for("⌘⇧P", Platform::Linux), "Ctrl+Shift+P");
        assert_eq!(display_for("⌃⇥", Platform::Linux), "Ctrl+Tab");
        assert_eq!(display_for("F12", Platform::Linux), "F12");
        assert_eq!(display_for("↩", Platform::Linux), "Enter");
    }

    #[test]
    fn single_chord_function_leaves_prose_alone() {
        assert_eq!(display_for("⌘↩ / ⌃↩", Platform::Linux), "⌘↩ / ⌃↩");
    }

    fn text_on_linux(text: &str) -> String {
        translate(text, Platform::Linux)
    }

    #[test]
    fn sentences_translate_each_shortcut() {
        assert_eq!(text_on_linux("Close Tab · ⌘W"), "Close Tab · Ctrl+W");
        assert_eq!(text_on_linux("⌘⇧O opens"), "Ctrl+Shift+O opens");
        assert_eq!(
            text_on_linux("return opens  ·  ⌘return beside"),
            "return opens  ·  Ctrl+Enter beside"
        );
        assert_eq!(text_on_linux("⌘-click to open"), "Ctrl-click to open");
        assert_eq!(text_on_linux("⌥↑ ⌥↓ section"), "Alt+↑ Alt+↓ section");
        assert_eq!(
            text_on_linux("Left-drag ⌘F or just type"),
            "Left-drag Ctrl+F or just type"
        );
        assert_eq!(text_on_linux("⌘1–⌘7"), "Ctrl+1–Ctrl+7");
        assert_eq!(text_on_linux("no keys here"), "no keys here");
    }
}
