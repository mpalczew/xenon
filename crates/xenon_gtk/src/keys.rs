//! The key map: `xenon_keymap`'s Linux defaults plus the user's `keymap.json`,
//! reduced to GTK accelerators for the actions this frontend has.

use std::collections::HashSet;

use xenon_keymap::{Entry, Platform};

/// Xenon action name -> GTK action name.
const ACTIONS: &[(&str, &str)] = &[
    ("xenon::NewTerminal", "app.new-terminal"),
    ("xenon::NewWorkspace", "app.new-workspace"),
    ("xenon::AddWorkspace", "app.open-workspace"),
    ("xenon::CloseEditor", "app.close-tab"),
    ("xenon::CloseOtherTabs", "app.close-other-tabs"),
    ("xenon::NextTab", "app.next-tab"),
    ("xenon::PrevTab", "app.prev-tab"),
    ("xenon::FocusNextPane", "app.next-pane"),
    ("xenon::NextWorkspace", "app.next-workspace"),
    ("xenon::PrevWorkspace", "app.prev-workspace"),
    ("xenon::CloseWorkspace", "app.close-workspace"),
    ("xenon::SplitRight", "app.split-right"),
    ("xenon::SplitDown", "app.split-down"),
    ("xenon::ReserveEmptyPaneRight", "app.park-pane"),
    ("xenon::FocusTerminal", "app.focus-terminal"),
    ("xenon::FocusBrowser", "app.focus-sidebar"),
    ("xenon::ToggleSidebar", "app.toggle-sidebar"),
    ("xenon_terminal::Find", "app.find"),
    ("xenon_terminal::FindNext", "app.find-next"),
    ("xenon_terminal::FindPrevious", "app.find-prev"),
    ("xenon_clipboard::Copy", "app.copy"),
    ("xenon_clipboard::Paste", "app.paste"),
    ("xenon::IncreaseFontSize", "app.zoom-in"),
    ("xenon::DecreaseFontSize", "app.zoom-out"),
    ("xenon::ResetFontSize", "app.zoom-reset"),
    ("xenon::Quit", "app.quit"),
];

/// Keys this frontend adds because the shared map leaves them to the terminal
/// (Ctrl+B reaches the shell while a terminal has focus).
const EXTRA: &[(&str, &str)] = &[("ctrl-alt-b", "app.toggle-sidebar")];

/// GTK key name for a gpui key token.
fn key_name(key: &str) -> Option<String> {
    let named = match key {
        "tab" => "Tab",
        "enter" => "Return",
        "escape" => "Escape",
        "backspace" => "BackSpace",
        "up" => "Up",
        "down" => "Down",
        "left" => "Left",
        "right" => "Right",
        "`" => "grave",
        "\\" => "backslash",
        "[" => "bracketleft",
        "]" => "bracketright",
        "=" => "equal",
        "+" => "plus",
        "-" => "minus",
        "_" => "underscore",
        "." => "period",
        "," => "comma",
        "/" => "slash",
        "?" => "question",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    let one_char = key.chars().count() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric());
    let function =
        key.len() > 1 && key.starts_with('f') && key[1..].chars().all(|c| c.is_ascii_digit());
    if function {
        return Some(key.to_uppercase());
    }
    one_char.then(|| key.to_string())
}

/// `ctrl-alt-shift-t` -> `<Control><Alt><Shift>t`. One keystroke only.
pub(crate) fn accelerator(keystroke: &str) -> Option<String> {
    if keystroke.contains(' ') {
        return None;
    }
    let mut out = String::new();
    let mut rest = keystroke;
    while let Some((head, tail)) = rest.split_once('-') {
        let modifier = match head {
            "ctrl" => "<Control>",
            "alt" => "<Alt>",
            "shift" => "<Shift>",
            "cmd" | "super" | "win" => "<Super>",
            _ => break,
        };
        if tail.is_empty() {
            break;
        }
        out.push_str(modifier);
        rest = tail;
    }
    out.push_str(&key_name(rest)?);
    Some(out)
}

/// Shifted keys arrive as a different keyval (`ctrl-shift-\` is `bar`); list
/// that spelling too so the chord matches on a US layout.
fn shifted_variant(accel: &str) -> Option<String> {
    let shifted = [
        ("backslash", "bar"),
        ("bracketleft", "braceleft"),
        ("bracketright", "braceright"),
        ("equal", "plus"),
        ("minus", "underscore"),
    ];
    if !accel.contains("<Shift>") {
        return None;
    }
    let (plain, bar) = shifted.iter().find(|(plain, _)| accel.ends_with(plain))?;
    let head = accel[..accel.len() - plain.len()].replace("<Shift>", "");
    Some(format!("{head}{bar}"))
}

/// Entries from the user's file: a JSON array of `{context?, bindings}`.
pub(crate) fn user_entries(text: &str) -> Vec<Entry> {
    let Ok(value) = serde_json_lenient::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for section in value.as_array().into_iter().flatten() {
        let context = section.get("context").and_then(|c| c.as_str());
        let Some(bindings) = section.get("bindings").and_then(|b| b.as_object()) else {
            continue;
        };
        for (keystroke, action) in bindings {
            let name = match action {
                serde_json::Value::String(s) => Some(s.as_str()),
                serde_json::Value::Array(a) => a.first().and_then(|s| s.as_str()),
                _ => None,
            };
            entries.push(Entry::new(context, keystroke, name));
        }
    }
    entries
}

/// Every (GTK action, accelerators) pair for the current keymap.
pub(crate) fn bindings(user: &[Entry]) -> Vec<(&'static str, Vec<String>)> {
    let mut entries = xenon_keymap::defaults(Platform::Linux);
    entries.extend(user.iter().cloned());
    // Keys the focused terminal hands to the shell stay out of the app map.
    let terminal_free: HashSet<&str> = entries
        .iter()
        .filter(|e| e.context.as_deref() == Some("Terminal") && e.action.is_none())
        .map(|e| e.keystroke.as_str())
        .collect();
    let mut out = Vec::new();
    for (name, gtk_action) in ACTIONS {
        let mut accels: Vec<String> = Vec::new();
        for entry in entries.iter().filter(|e| e.action.as_deref() == Some(name)) {
            let context_ok = matches!(entry.context.as_deref(), None | Some("Terminal"));
            let current = latest(&entries, entry);
            if !context_ok || !current || terminal_free.contains(entry.keystroke.as_str()) {
                continue;
            }
            if let Some(accel) = accelerator(&entry.keystroke) {
                accels.extend(shifted_variant(&accel));
                accels.push(accel);
            }
        }
        for (key, action) in EXTRA {
            if action == gtk_action {
                accels.extend(accelerator(key));
            }
        }
        accels.dedup();
        out.push((*gtk_action, accels));
    }
    out
}

/// True when no later entry rebinds the same keystroke in the same context.
fn latest(entries: &[Entry], entry: &Entry) -> bool {
    entries
        .iter()
        .rfind(|e| e.context == entry.context && e.keystroke == entry.keystroke)
        .is_some_and(|last| last.action == entry.action)
}
