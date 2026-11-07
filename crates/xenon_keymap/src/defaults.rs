//! Default bindings. One table is written with `cmd`; Linux swaps `cmd` for
//! `ctrl`, moves the few keys that collide, and adds terminal pass-through.
//! Unconditional bindings tie with `Terminal`-context ones on the focused
//! terminal, and the later entry wins, so context sections come after the
//! global one and Linux extras come last.

use crate::{Entry, Platform};

type Row = (Option<&'static str>, &'static str, &'static str);

const SHARED: &[Row] = &[
    (None, "cmd-n", "xenon::NewTerminal"),
    (None, "cmd-alt-n", "xenon::NewWorkspace"),
    (None, "cmd-p", "xenon::FilePalette"),
    (None, "cmd-shift-o", "xenon::AddWorkspace"),
    (None, "cmd-shift-r", "xenon::RunTask"),
    (None, "cmd-shift-b", "xenon::RunBuildTask"),
    (None, "cmd-alt-enter", "xenon::AllowFolderTasks"),
    (None, "cmd-alt-backspace", "xenon::DenyFolderTasks"),
    (None, "cmd-shift-k", "xenon::CaptureWorklist"),
    (None, "cmd-alt-k", "xenon::OpenWorklist"),
    (None, "cmd-z", "xenon::UndoToast"),
    (None, "cmd-1", "xenon::FocusTerminal"),
    (None, "cmd-3", "xenon::FocusBrowser"),
    (None, "cmd-alt-g", "xenon::FocusGit"),
    (None, "cmd-alt-shift-d", "xenon::CycleDiffLayout"),
    (None, "ctrl-`", "xenon::FocusNextPane"),
    (None, "cmd-alt-down", "xenon::NextWorkspace"),
    (None, "cmd-alt-up", "xenon::PrevWorkspace"),
    (None, "cmd-alt-w", "xenon::CloseWorkspace"),
    (None, "ctrl-tab", "xenon::NextTab"),
    (None, "ctrl-shift-tab", "xenon::PrevTab"),
    (None, "cmd-shift-]", "xenon::NextTab"),
    (None, "cmd-shift-[", "xenon::PrevTab"),
    (None, "cmd-w", "xenon::CloseEditor"),
    (None, "cmd-alt-shift-w", "xenon::CloseOtherTabs"),
    (None, "cmd-s", "xenon::Save"),
    (None, "cmd-c", "xenon_clipboard::Copy"),
    (None, "cmd-shift-c", "xenon_clipboard::CopyClean"),
    (None, "cmd-v", "xenon_clipboard::Paste"),
    (None, "cmd-b", "xenon::ToggleSidebar"),
    (None, "cmd-\\", "xenon::SplitRight"),
    (None, "cmd-shift-\\", "xenon::SplitDown"),
    (None, "cmd-alt-\\", "xenon::ReserveEmptyPaneRight"),
    (None, "cmd-e", "xenon::ToggleBrowser"),
    (None, "cmd-,", "xenon::ToggleSettings"),
    (None, "cmd-shift-m", "xenon::ConnectPhone"),
    (None, "cmd-=", "xenon::IncreaseFontSize"),
    (None, "cmd-+", "xenon::IncreaseFontSize"),
    (None, "cmd--", "xenon::DecreaseFontSize"),
    (None, "cmd-0", "xenon::ResetFontSize"),
    (None, "cmd-q", "xenon::Quit"),
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
    (None, "xenon::UndoToast", "ctrl-z", "ctrl-alt-z"),
    (
        None,
        "xenon_clipboard::CopyClean",
        "ctrl-shift-c",
        "ctrl-shift-y",
    ),
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
    ("ctrl-c", None),
    ("ctrl-x", None),
    ("ctrl-b", None),
    ("ctrl-e", None),
    ("ctrl-o", None),
    ("ctrl-s", None),
];

pub fn defaults(platform: Platform) -> Vec<Entry> {
    match platform {
        Platform::Linux => linux(shared().map(substitute_cmd).map(move_if_listed).collect()),
    }
}

/// The table as written, with `cmd` as the primary modifier.
pub(crate) fn shared() -> impl Iterator<Item = Entry> {
    SHARED
        .iter()
        .map(|&(context, keystroke, action)| Entry::new(context, keystroke, Some(action)))
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
