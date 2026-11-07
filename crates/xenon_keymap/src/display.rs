//! Shortcut text for UI chrome. Call sites hand in the glyph string they
//! have always written (`⌘⇧P`); this returns what the current platform and the
//! user's keymap actually bind.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::{Chord, Platform};

fn overrides() -> &'static RwLock<HashMap<String, String>> {
    static OVERRIDES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    OVERRIDES.get_or_init(Default::default)
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
        Some(_) => display_for(&normalized, platform),
        None => modifier_names(token),
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
        (None, Platform::Linux) => chord.for_linux().format(),
    }
}
