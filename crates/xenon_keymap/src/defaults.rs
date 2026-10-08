//! Default bindings. One table is written with `cmd`; Linux swaps `cmd` for
//! `ctrl`, moves the few keys that collide, and adds terminal pass-through.
//! Unconditional bindings tie with `Terminal`-context ones on the focused
//! terminal, and the later entry wins, so context sections come after the
//! global one and Linux extras come last.

use crate::{Entry, Platform};

type Row = (Option<&'static str>, &'static str, &'static str);

const SHARED: &[Row] = &[
    (None, "cmd-n", "xenon::NewTerminal"),
    (None, "cmd-shift-n", "xenon::NewFile"),
    (None, "cmd-alt-n", "xenon::NewWorkspace"),
    (None, "cmd-alt-shift-f", "xenon::NewFolder"),
    (None, "cmd-o", "xenon::OpenFile"),
    (None, "cmd-p", "xenon::FilePalette"),
    (None, "cmd-shift-o", "xenon::AddWorkspace"),
    (None, "cmd-shift-r", "xenon::RunTask"),
    (None, "cmd-shift-k", "xenon::CaptureWorklist"),
    (None, "cmd-alt-k", "xenon::OpenWorklist"),
    (None, "cmd-z", "xenon::UndoToast"),
    (None, "cmd-.", "xenon::DismissToast"),
    (None, "cmd-shift-p", "xenon::CommandPalette"),
    (None, "cmd-?", "xenon::KeyboardHelp"),
    (None, "cmd-shift-/", "xenon::KeyboardHelp"),
    (None, "cmd-1", "xenon::FocusTerminal"),
    (None, "cmd-2", "xenon::FocusEditor"),
    (None, "cmd-3", "xenon::FocusBrowser"),
    (None, "ctrl-`", "xenon::FocusNextPane"),
    (None, "cmd-alt-down", "xenon::NextWorkspace"),
    (None, "cmd-alt-up", "xenon::PrevWorkspace"),
    (None, "cmd-alt-w", "xenon::CloseWorkspace"),
    (None, "ctrl-tab", "xenon::NextTab"),
    (None, "ctrl-shift-tab", "xenon::PrevTab"),
    (None, "cmd-shift-]", "xenon::NextTab"),
    (None, "cmd-shift-[", "xenon::PrevTab"),
    (None, "cmd-[", "xenon::GoBack"),
    (None, "cmd-]", "xenon::GoForward"),
    (None, "cmd-w", "xenon::CloseEditor"),
    (None, "cmd-alt-shift-w", "xenon::CloseOtherTabs"),
    (None, "cmd-alt-shift-c", "xenon::CopyPath"),
    (None, "cmd-alt-shift-r", "xenon::CopyRelativePath"),
    (None, "cmd-alt-r", "xenon::RevealInFinder"),
    (None, "cmd-alt-o", "xenon::OpenInDefaultApp"),
    (None, "cmd-s", "xenon::Save"),
    (None, "cmd-shift-s", "xenon::SaveAs"),
    (None, "cmd-x", "xenon_clipboard::Cut"),
    (None, "cmd-c", "xenon_clipboard::Copy"),
    (None, "cmd-shift-c", "xenon_clipboard::CopyClean"),
    (None, "cmd-alt-c", "xenon_clipboard::CopyCode"),
    (None, "cmd-v", "xenon_clipboard::Paste"),
    (None, "cmd-b", "xenon::ToggleSidebar"),
    (None, "cmd-j", "xenon::ToggleTerminal"),
    (None, "cmd-shift-e", "xenon::ToggleEditor"),
    (None, "cmd-\\", "xenon::SplitRight"),
    (None, "cmd-shift-\\", "xenon::SplitDown"),
    (None, "cmd-alt-\\", "xenon::ReserveEmptyPaneRight"),
    (None, "cmd-e", "xenon::ToggleBrowser"),
    (None, "cmd-,", "xenon::ToggleSettings"),
    (None, "cmd-alt-t", "xenon::ToggleThemes"),
    (None, "cmd-shift-v", "xenon::TogglePreview"),
    (None, "alt-z", "xenon::ToggleSoftWrap"),
    (None, "cmd-alt-m", "xenon::ToggleMemory"),
    (None, "cmd-shift-m", "xenon::ConnectPhone"),
    (None, "cmd-=", "xenon::IncreaseFontSize"),
    (None, "cmd-+", "xenon::IncreaseFontSize"),
    (None, "cmd--", "xenon::DecreaseFontSize"),
    (None, "cmd-0", "xenon::ResetFontSize"),
    (None, "cmd-q", "xenon::Quit"),
    (Some("Editor"), "f12", "xenon::GoToDefinition"),
    (Some("Editor"), "f8", "xenon::NextDiagnostic"),
    (Some("Editor"), "shift-f8", "xenon::PreviousDiagnostic"),
    (Some("Editor"), "cmd-a", "xenon_clipboard::SelectAll"),
    (Some("Editor"), "cmd-f", "xenon_editor::Find"),
    (Some("Editor"), "cmd-g", "xenon_editor::FindNext"),
    (Some("Editor"), "cmd-shift-g", "xenon_editor::FindPrevious"),
    (Some("EditorFind"), "cmd-f", "xenon_editor::Find"),
    (Some("EditorFind"), "cmd-g", "xenon_editor::FindNext"),
    (
        Some("EditorFind"),
        "cmd-shift-g",
        "xenon_editor::FindPrevious",
    ),
    (Some("Terminal"), "cmd-f", "xenon_terminal::Find"),
    (Some("Terminal"), "cmd-g", "xenon_terminal::FindNext"),
    (
        Some("Terminal"),
        "cmd-shift-g",
        "xenon_terminal::FindPrevious",
    ),
    (Some("TerminalFind"), "cmd-f", "xenon_terminal::Find"),
    (Some("TerminalFind"), "cmd-g", "xenon_terminal::FindNext"),
    (
        Some("TerminalFind"),
        "cmd-shift-g",
        "xenon_terminal::FindPrevious",
    ),
];

/// Linux keys that collide once `cmd` becomes `ctrl`: (context, action,
/// keystroke after substitution, replacement).
const LINUX_MOVED: &[(Option<&str>, &str, &str, &str)] = &[
    // ctrl-z is the shell's suspend; undo-capture keeps a chord of its own.
    (None, "xenon::UndoToast", "ctrl-z", "ctrl-alt-z"),
    // ctrl-shift-c is copy in a Linux terminal (zed does the same).
    (
        None,
        "xenon_clipboard::CopyClean",
        "ctrl-shift-c",
        "ctrl-shift-y",
    ),
    // ctrl-alt-t opens a terminal on Ubuntu before any app sees it.
    (
        None,
        "xenon::ToggleThemes",
        "ctrl-alt-t",
        "ctrl-alt-shift-t",
    ),
    // ctrl-[ is Escape (vim, terminals), so history moves like zed's.
    (None, "xenon::GoBack", "ctrl-[", "ctrl-alt--"),
    (None, "xenon::GoForward", "ctrl-]", "ctrl-alt-_"),
    // Terminal find follows zed; ctrl-f / ctrl-g belong to readline and TUIs.
    (
        Some("Terminal"),
        "xenon_terminal::Find",
        "ctrl-f",
        "ctrl-shift-f",
    ),
    (Some("Terminal"), "xenon_terminal::FindNext", "ctrl-g", "f3"),
    (
        Some("Terminal"),
        "xenon_terminal::FindPrevious",
        "ctrl-shift-g",
        "shift-f3",
    ),
];

/// Linux terminal bindings, added after everything else.
const LINUX_TERMINAL: &[(&str, Option<&str>)] = &[
    ("ctrl-shift-c", Some("xenon_clipboard::Copy")),
    ("ctrl-shift-v", Some("xenon_clipboard::Paste")),
    // `null` here lets the key reach the PTY instead of the app binding.
    ("ctrl-c", None),
    ("ctrl-x", None),
    ("ctrl-v", None),
    ("ctrl-b", None),
    ("ctrl-e", None),
    ("ctrl-o", None),
    ("ctrl-s", None),
];

pub fn defaults(platform: Platform) -> Vec<Entry> {
    let shared = SHARED
        .iter()
        .map(|&(context, keystroke, action)| Entry::new(context, keystroke, Some(action)));
    match platform {
        Platform::Mac => shared.collect(),
        Platform::Linux => linux(shared.map(substitute_cmd).map(move_if_listed).collect()),
    }
}

/// `cmd` only ever appears as a modifier token in the shared table.
fn substitute_cmd(mut entry: Entry) -> Entry {
    entry.keystroke = entry
        .keystroke
        .split('-')
        .map(|part| if part == "cmd" { "ctrl" } else { part })
        .collect::<Vec<_>>()
        .join("-");
    entry
}

fn move_if_listed(mut entry: Entry) -> Entry {
    for &(context, action, from, to) in LINUX_MOVED {
        if entry.context.as_deref() == context
            && entry.action.as_deref() == Some(action)
            && entry.keystroke == from
        {
            entry.keystroke = to.to_owned();
        }
    }
    entry
}

fn linux(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.extend(
        LINUX_TERMINAL
            .iter()
            .map(|&(keystroke, action)| Entry::new(Some("Terminal"), keystroke, action)),
    );
    entries
}

/// zed-shaped keymap JSON: consecutive entries with one context share a section.
pub fn to_json(entries: &[Entry]) -> String {
    let mut sections: Vec<serde_json::Value> = Vec::new();
    let mut previous: Option<&Option<String>> = None;
    for entry in entries {
        if previous != Some(&entry.context) {
            sections.push(new_section(&entry.context));
            previous = Some(&entry.context);
        }
        let bindings = sections
            .last_mut()
            .and_then(|section| section.get_mut("bindings"))
            .and_then(|bindings| bindings.as_object_mut());
        if let Some(bindings) = bindings {
            bindings.insert(entry.keystroke.clone(), action_value(&entry.action));
        }
    }
    serde_json::Value::Array(sections).to_string()
}

fn new_section(context: &Option<String>) -> serde_json::Value {
    let mut section = serde_json::Map::new();
    if let Some(context) = context {
        section.insert("context".into(), context.clone().into());
    }
    section.insert("bindings".into(), serde_json::Map::new().into());
    section.into()
}

fn action_value(action: &Option<String>) -> serde_json::Value {
    match action {
        Some(name) => name.clone().into(),
        None => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};

    fn flat(entries: &[Entry]) -> BTreeSet<String> {
        entries
            .iter()
            .map(|e| {
                format!(
                    "{}\t{}\t{}",
                    e.context.as_deref().unwrap_or("-"),
                    e.keystroke,
                    e.action.as_deref().unwrap_or("null")
                )
            })
            .collect()
    }

    #[test]
    fn mac_defaults_match_the_shipped_bindings() {
        let golden: BTreeSet<String> = include_str!("../tests/mac_defaults.golden")
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(flat(&defaults(Platform::Mac)), golden);
    }

    #[test]
    fn linux_has_no_cmd_and_no_super() {
        for entry in defaults(Platform::Linux) {
            assert!(!entry.keystroke.contains("cmd"), "{entry:?}");
        }
    }

    #[test]
    fn linux_swaps_cmd_for_ctrl() {
        let linux = flat(&defaults(Platform::Linux));
        for want in [
            "-\tctrl-p\txenon::FilePalette",
            "-\tctrl-shift-p\txenon::CommandPalette",
            "-\tctrl-n\txenon::NewTerminal",
            "-\tctrl-w\txenon::CloseEditor",
            "-\tctrl-s\txenon::Save",
            "-\tctrl-1\txenon::FocusTerminal",
            "-\tctrl-q\txenon::Quit",
            "-\tctrl-tab\txenon::NextTab",
        ] {
            assert!(linux.contains(want), "missing {want}");
        }
    }

    #[test]
    fn linux_terminal_keeps_shell_keys() {
        let linux = defaults(Platform::Linux);
        for key in ["ctrl-c", "ctrl-b", "ctrl-e", "ctrl-o", "ctrl-s"] {
            let found = linux
                .iter()
                .rev()
                .find(|e| e.context.as_deref() == Some("Terminal") && e.keystroke == key);
            assert_eq!(found.map(|e| e.action.clone()), Some(None), "{key}");
        }
        assert!(flat(&linux).contains("Terminal\tctrl-shift-c\txenon_clipboard::Copy"));
    }

    #[test]
    fn linux_has_no_conflicting_bindings() {
        let mut seen: HashMap<(Option<String>, String), Option<String>> = HashMap::new();
        for entry in defaults(Platform::Linux) {
            let key = (entry.context.clone(), entry.keystroke.clone());
            if let Some(old) = seen.insert(key, entry.action.clone()) {
                assert_eq!(old, entry.action, "duplicate {entry:?}");
            }
        }
    }

    #[test]
    fn json_groups_entries_by_context() {
        let json = to_json(&defaults(Platform::Mac));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let sections = value.as_array().unwrap();
        assert_eq!(sections.len(), 5);
        assert!(sections[0].get("context").is_none());
        assert_eq!(sections[1]["context"], "Editor");
    }
}
