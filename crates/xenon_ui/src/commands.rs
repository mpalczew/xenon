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
    NewWorkspace,
    OpenWorkspace,
    OpenFile,
    NewFile,
    NewFolder,
    GoToFile,
    RunTask,
    Save,
    SaveAs,
    FindInFile,
    FindNext,
    FindPrevious,
    CloseFocusedTab,
    CloseOtherTabs,
    CloseWorkspace,
    CopyPath,
    CopyRelativePath,
    CopyClean,
    CopyCode,
    RevealInFinder,
    OpenInDefaultApp,
    FocusTerminal,
    FocusEditor,
    FocusBrowser,
    FocusNextPane,
    NextWorkspace,
    PrevWorkspace,
    NextTab,
    PrevTab,
    GoBack,
    GoForward,
    GoToDefinition,
    NextDiagnostic,
    PreviousDiagnostic,
    ToggleSidebar,
    ToggleBrowser,
    ToggleTerminal,
    ToggleEditor,
    SplitRight,
    SplitDown,
    ReserveEmptyPaneRight,
    RemoveEmptyPane,
    ToggleSettings,
    ToggleThemes,
    ToggleMobileRemote,
    TogglePreview,
    ToggleMemory,
    CommandPalette,
    KeyboardHelp,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CaptureWorklist,
    OpenWorklist,
    Copy,
    Cut,
    Paste,
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

const fn all_commands() -> [CommandEntry; 58] {
    [
        cmd!(NextWorkspace, "Next Workspace", "⌘⌥↓", "Navigate"),
        cmd!(PrevWorkspace, "Previous Workspace", "⌘⌥↑", "Navigate"),
        cmd!(GoToFile, "Go to File…", "⌘P", "Navigate"),
        cmd!(OpenWorkspace, "Open Workspace…", "⌘⇧O", "Navigate"),
        cmd!(CommandPalette, "Command Palette…", "⌘⇧P", "Navigate"),
        cmd!(KeyboardHelp, "Keyboard Shortcuts", "⌘⇧/", "Navigate"),
        cmd!(GoBack, "Go Back", "⌘[", "Navigate"),
        cmd!(GoForward, "Go Forward", "⌘]", "Navigate"),
        cmd!(GoToDefinition, "Go to Definition", "F12", "Navigate"),
        cmd!(NextDiagnostic, "Next Diagnostic", "F8", "Navigate"),
        cmd!(PreviousDiagnostic, "Previous Diagnostic", "⇧F8", "Navigate"),
        cmd!(FocusTerminal, "Focus Terminal", "⌘1", "Focus"),
        cmd!(FocusEditor, "Focus Editor", "⌘2", "Focus"),
        cmd!(FocusBrowser, "Focus File Tree", "⌘3", "Focus"),
        cmd!(FocusNextPane, "Focus Next Pane", "⌃`", "Focus"),
        cmd!(NextTab, "Next Tab", "⌃⇥", "Focus"),
        cmd!(PrevTab, "Previous Tab", "⌃⇧⇥", "Focus"),
        cmd!(Save, "Save", "⌘S", "Edit"),
        cmd!(SaveAs, "Save As…", "⌘⇧S", "Edit"),
        cmd!(FindInFile, "Find…", "⌘F", "Edit"),
        cmd!(FindNext, "Find Next", "⌘G", "Edit"),
        cmd!(FindPrevious, "Find Previous", "⌘⇧G", "Edit"),
        cmd!(CloseFocusedTab, "Close Tab", "⌘W", "Edit"),
        cmd!(CloseOtherTabs, "Close Other Tabs", "⌘⌥⇧W", "Edit"),
        cmd!(CloseWorkspace, "Close Workspace", "⌘⌥W", "Edit"),
        cmd!(CopyPath, "Copy Path", "⌘⌥⇧C", "Edit"),
        cmd!(CopyRelativePath, "Copy Relative Path", "⌘⌥⇧R", "Edit"),
        cmd!(CopyClean, "Copy Clean", "⌘⇧C", "Edit"),
        cmd!(CopyCode, "Copy Code", "⌘⌥C", "Edit"),
        cmd!(RevealInFinder, "Reveal in Finder", "⌘⌥R", "Edit"),
        cmd!(OpenInDefaultApp, "Open in Default App", "⌘⌥O", "Edit"),
        cmd!(NewTerminal, "New Terminal", "⌘N", "Create"),
        cmd!(NewWorkspace, "New Workspace…", "⌘⌥N", "Create"),
        cmd!(NewFile, "New File…", "⌘⇧N", "Create"),
        cmd!(NewFolder, "New Folder…", "⌘⌥⇧F", "Create"),
        cmd!(OpenFile, "Open File…", "⌘O", "Create"),
        cmd!(RunTask, "Run Task…", "⌘⇧R", "Create"),
        cmd!(ToggleSidebar, "Toggle Sidebar", "⌘B", "View"),
        cmd!(ToggleBrowser, "Toggle File Tree", "⌘E", "View"),
        cmd!(ToggleTerminal, "Focus/New Terminal", "⌘J", "View"),
        cmd!(ToggleEditor, "Focus Editor", "⌘⇧E", "View"),
        cmd!(SplitRight, "Split Right", "⌘\\", "View"),
        cmd!(SplitDown, "Split Down", "⌘⇧\\", "View"),
        cmd!(
            ReserveEmptyPaneRight,
            "Reserve Pane to the Right",
            "⌘⌥\\",
            "View"
        ),
        cmd!(RemoveEmptyPane, "Remove Empty Pane", "", "View"),
        cmd!(ToggleSettings, "Settings", "⌘,", "View"),
        cmd!(ToggleThemes, "Themes…", "⌘⌥T", "View"),
        cmd!(ToggleMobileRemote, "Toggle Mobile Remote", "", "View"),
        cmd!(TogglePreview, "Toggle Markdown Preview", "⌘⇧V", "View"),
        cmd!(ToggleMemory, "Memory Diagnostics", "⌘⌥M", "View"),
        cmd!(ZoomIn, "Zoom In", "⌘=", "View"),
        cmd!(ZoomOut, "Zoom Out", "⌘-", "View"),
        cmd!(ZoomReset, "Reset Zoom", "⌘0", "View"),
        cmd!(CaptureWorklist, "Add to Worklist", "⌘⇧K", "Create"),
        cmd!(OpenWorklist, "Open Worklist", "⌘⌥K", "Navigate"),
        cmd!(Copy, "Copy", "⌘C", "Edit"),
        cmd!(Cut, "Cut", "⌘X", "Edit"),
        cmd!(Paste, "Paste", "⌘V", "Edit"),
    ]
}

/// Full catalog for palette + help.
pub fn catalog() -> &'static [CommandEntry] {
    static ENTRIES: [CommandEntry; 58] = all_commands();
    &ENTRIES
}

pub fn entry(id: CommandId) -> CommandEntry {
    catalog()
        .iter()
        .copied()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("missing command {id:?}"))
}

/// Shortcut shown on in-app menu rows. Panics if the command has no keys.
pub fn shortcut(id: CommandId) -> xenon_design_system::Shortcut {
    xenon_design_system::Shortcut::new(entry(id).keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let mut seen = Vec::new();
        for entry in catalog() {
            assert!(
                !seen.contains(&entry.id),
                "duplicate catalog id {:?}",
                entry.id
            );
            seen.push(entry.id);
        }
    }

    #[test]
    fn menu_commands_have_shortcuts() {
        for id in [
            CommandId::NewWorkspace,
            CommandId::OpenWorkspace,
            CommandId::CloseFocusedTab,
            CommandId::CloseOtherTabs,
            CommandId::CopyPath,
            CommandId::CopyRelativePath,
            CommandId::CopyClean,
            CommandId::CopyCode,
            CommandId::RevealInFinder,
            CommandId::OpenInDefaultApp,
            CommandId::NewFile,
            CommandId::NewFolder,
            CommandId::NewTerminal,
        ] {
            assert!(!entry(id).keys.is_empty(), "{id:?} needs a shortcut");
            let _ = shortcut(id);
        }
    }

    #[test]
    fn worklist_shortcuts_match_the_approved_pair() {
        assert_eq!(entry(CommandId::CaptureWorklist).keys, "⌘⇧K");
        assert_eq!(entry(CommandId::OpenWorklist).keys, "⌘⌥K");
    }
}
