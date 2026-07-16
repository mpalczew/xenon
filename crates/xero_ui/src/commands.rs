//! Static command catalog: labels, key chords, and ids for the command palette
//! and keyboard help. Actions themselves live in `lib.rs` (gpui `actions!`).

/// One row in the command palette / cheatsheet.
#[derive(Clone, Copy, Debug)]
pub struct CommandEntry {
    pub id: CommandId,
    pub label: &'static str,
    pub keys: &'static str,
    pub group: &'static str,
}

/// Executable command ids (palette + keyboard help).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandId {
    NewTerminal,
    NewStream,
    OpenWorkspace,
    OpenFile,
    GoToFile,
    RunTask,
    Save,
    CloseFocusedTab,
    CloseStream,
    CloseWorkspace,
    FocusTerminal,
    FocusEditor,
    FocusBrowser,
    FocusNextPane,
    NextStream,
    PrevStream,
    StreamPalette,
    NextWorkspace,
    PrevWorkspace,
    NextTab,
    PrevTab,
    ToggleSidebar,
    ToggleBrowser,
    ToggleTerminal,
    ToggleEditor,
    ToggleSettings,
    CommandPalette,
    KeyboardHelp,
    MoveTabMenu,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

macro_rules! cmd {
    ($id:ident, $label:expr, $keys:expr, $group:expr) => {
        CommandEntry {
            id: CommandId::$id,
            label: $label,
            keys: $keys,
            group: $group,
        }
    };
}

const fn all_commands() -> [CommandEntry; 32] {
    [
        cmd!(StreamPalette, "Go to Stream…", "⌘T", "Navigate"),
        cmd!(NextStream, "Next Stream", "⌘⌥↓", "Navigate"),
        cmd!(PrevStream, "Previous Stream", "⌘⌥↑", "Navigate"),
        cmd!(NextWorkspace, "Next Workspace", "⌘⌥→", "Navigate"),
        cmd!(PrevWorkspace, "Previous Workspace", "⌘⌥←", "Navigate"),
        cmd!(GoToFile, "Go to File…", "⌘P", "Navigate"),
        cmd!(OpenWorkspace, "Open Workspace…", "⌘⇧O", "Navigate"),
        cmd!(CommandPalette, "Command Palette…", "⌘⇧P", "Navigate"),
        cmd!(KeyboardHelp, "Keyboard Shortcuts", "⌘⇧/", "Navigate"),
        cmd!(FocusTerminal, "Focus Terminal", "⌘1", "Focus"),
        cmd!(FocusEditor, "Focus Editor", "⌘2", "Focus"),
        cmd!(FocusBrowser, "Focus File Tree", "⌘3", "Focus"),
        cmd!(FocusNextPane, "Focus Next Pane", "⌃`", "Focus"),
        cmd!(NextTab, "Next Tab", "⌘⇧]", "Focus"),
        cmd!(PrevTab, "Previous Tab", "⌘⇧[", "Focus"),
        cmd!(Save, "Save", "⌘S", "Edit"),
        cmd!(CloseFocusedTab, "Close Tab", "⌘W", "Edit"),
        cmd!(CloseStream, "Close Stream", "⌘⇧W", "Edit"),
        cmd!(CloseWorkspace, "Close Workspace", "⌘⌥W", "Edit"),
        cmd!(MoveTabMenu, "Move Tab to Stream…", "⌘⇧M", "Edit"),
        cmd!(NewTerminal, "New Terminal", "⌘N", "Create"),
        cmd!(NewStream, "New Stream", "⌘⇧N", "Create"),
        cmd!(OpenFile, "Open File…", "⌘O", "Create"),
        cmd!(RunTask, "Run Task…", "⌘⇧R", "Create"),
        cmd!(ToggleSidebar, "Toggle Sidebar", "⌘B", "View"),
        cmd!(ToggleBrowser, "Toggle File Tree", "⌘E", "View"),
        cmd!(ToggleTerminal, "Toggle Terminal", "⌘J", "View"),
        cmd!(ToggleEditor, "Toggle Editor", "⌘⇧E", "View"),
        cmd!(ToggleSettings, "Settings", "⌘,", "View"),
        cmd!(ZoomIn, "Zoom In", "⌘=", "View"),
        cmd!(ZoomOut, "Zoom Out", "⌘-", "View"),
        cmd!(ZoomReset, "Reset Zoom", "⌘0", "View"),
    ]
}

/// Full catalog for palette + help.
pub fn catalog() -> &'static [CommandEntry] {
    static ENTRIES: [CommandEntry; 32] = all_commands();
    &ENTRIES
}
