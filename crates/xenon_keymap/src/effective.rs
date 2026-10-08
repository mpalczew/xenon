//! What the user's file changes about displayed shortcuts.

use std::collections::HashMap;

use crate::chord::format_sequence;
use crate::{Chord, Entry, Platform, defaults};

type Applied = Vec<((Option<String>, String), Option<String>)>;

/// Later entries win per (context, keystroke); `null` leaves the key unbound.
fn apply(entries: &[Entry]) -> Applied {
    let mut applied: Applied = Vec::new();
    for entry in entries {
        let id = (entry.context.clone(), canonical(&entry.keystroke));
        applied.retain(|(existing, _)| *existing != id);
        applied.push((id, entry.action.clone()));
    }
    applied
}

fn canonical(keystroke: &str) -> String {
    keystroke
        .split_whitespace()
        .map(|part| Chord::parse(part).map_or_else(|| part.to_owned(), |c| c.canonical()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The keystroke an action shows in menus: the last unconditional binding,
/// else the last binding in any context.
fn primary(entries: &[Entry], action: &str) -> Option<String> {
    let applied = apply(entries);
    let bound = |(_, a): &&((Option<String>, String), Option<String>)| a.as_deref() == Some(action);
    let global = applied
        .iter()
        .filter(bound)
        .rfind(|((ctx, _), _)| ctx.is_none());
    let any = applied.iter().rfind(bound);
    global.or(any).map(|((_, keys), _)| keys.clone())
}

/// For each macOS default keystroke whose action the user rebound or
/// unbound: the text to show instead. Untouched defaults are absent, so the
/// shipped glyphs stay as written.
pub fn display_overrides(platform: Platform, user: &[Entry]) -> HashMap<String, String> {
    let base = defaults(platform);
    let merged: Vec<Entry> = base.iter().chain(user).cloned().collect();
    let mut out = HashMap::new();
    for entry in defaults(Platform::Mac) {
        let (Some(action), Some(chord)) = (entry.action.as_deref(), Chord::parse(&entry.keystroke))
        else {
            continue;
        };
        let now = primary(&merged, action);
        if now == primary(&base, action) {
            continue;
        }
        let text = now.map_or_else(String::new, |keys| format_sequence(&keys, platform));
        out.entry(chord.canonical()).or_insert(text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(keys: &str, action: &str) -> Entry {
        Entry::new(None, keys, Some(action))
    }

    #[test]
    fn untouched_defaults_have_no_overrides() {
        assert!(display_overrides(Platform::Linux, &[]).is_empty());
        assert!(display_overrides(Platform::Mac, &[]).is_empty());
    }

    #[test]
    fn rebinding_shows_the_new_key() {
        let user = [bind("ctrl-alt-p", "xenon::FilePalette")];
        let map = display_overrides(Platform::Linux, &user);
        assert_eq!(map.get("cmd-p").map(String::as_str), Some("Ctrl+Alt+P"));
    }

    #[test]
    fn rebinding_on_mac_uses_glyphs() {
        let user = [bind("cmd-alt-p", "xenon::FilePalette")];
        let map = display_overrides(Platform::Mac, &user);
        assert_eq!(map.get("cmd-p").map(String::as_str), Some("⌥⌘P"));
    }

    #[test]
    fn unbinding_shows_nothing() {
        let user = [Entry::new(None, "ctrl-p", None)];
        let map = display_overrides(Platform::Linux, &user);
        assert_eq!(map.get("cmd-p").map(String::as_str), Some(""));
    }
}
