//! The Xenon application shell: a collapsible workspace sidebar and a main panel
//! hosting a terminal and editor, with a cmd-p fuzzy finder.

mod app;
mod chrome;
mod command_palette;
mod commands;
mod dropdown;
mod file_browser;
mod finder;
mod git_dirt;
mod icons;
mod palette;
mod rename;
mod resize;
mod settings;
mod sidebar;
mod tabs;
mod task_picker;
mod toolbar;
mod workspace_discover;
mod workspace_picker;

pub use settings::SettingsView;

use gpui::{App, KeyBinding, actions};

pub use app::XenonApp;
pub(crate) use icons::preview_icon;
pub use xenon_settings::{Copy, Cut, Paste, SelectAll};

actions!(
    xenon,
    [
        NewTerminal,
        ToggleSidebar,
        ToggleTerminal,
        ToggleEditor,
        ToggleBrowser,
        OpenFile,
        AddWorkspace,
        FilePalette,
        RunTask,
        CloseEditor,
        Save,
        IncreaseFontSize,
        DecreaseFontSize,
        ResetFontSize,
        ToggleSettings,
        // Keyboard-first navigation
        FocusTerminal,
        FocusEditor,
        FocusBrowser,
        FocusNextPane,
        NextWorkspace,
        PrevWorkspace,
        CloseWorkspace,
        NextTab,
        PrevTab,
        CommandPalette,
        KeyboardHelp,
    ]
);

/// Bind the shell's keyboard shortcuts. Call once at startup.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        // Create
        KeyBinding::new("cmd-n", NewTerminal, None),
        // Open / jump
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-p", FilePalette, None),
        KeyBinding::new("cmd-shift-o", AddWorkspace, None),
        KeyBinding::new("cmd-shift-r", RunTask, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        // macOS delivers Shift+/ as `?` (same as Zed's `cmd-?` bindings).
        KeyBinding::new("cmd-?", KeyboardHelp, None),
        KeyBinding::new("cmd-shift-/", KeyboardHelp, None),
        // Focus panes
        KeyBinding::new("cmd-1", FocusTerminal, None),
        KeyBinding::new("cmd-2", FocusEditor, None),
        KeyBinding::new("cmd-3", FocusBrowser, None),
        KeyBinding::new("ctrl-`", FocusNextPane, None),
        // Workspaces (vertical list)
        KeyBinding::new("cmd-alt-down", NextWorkspace, None),
        KeyBinding::new("cmd-alt-up", PrevWorkspace, None),
        KeyBinding::new("cmd-alt-w", CloseWorkspace, None),
        // Tabs (ctrl-tab matches browser/IDE muscle memory)
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("cmd-shift-]", NextTab, None),
        KeyBinding::new("cmd-shift-[", PrevTab, None),
        KeyBinding::new("cmd-w", CloseEditor, None),
        // Edit
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-a", SelectAll, Some("Editor")),
        // In-buffer find (editor context; also works when find bar focused)
        KeyBinding::new("cmd-f", xenon_editor::Find, Some("Editor")),
        KeyBinding::new("cmd-f", xenon_editor::Find, Some("EditorFind")),
        KeyBinding::new("cmd-g", xenon_editor::FindNext, Some("Editor")),
        KeyBinding::new("cmd-g", xenon_editor::FindNext, Some("EditorFind")),
        KeyBinding::new("cmd-shift-g", xenon_editor::FindPrevious, Some("Editor")),
        KeyBinding::new(
            "cmd-shift-g",
            xenon_editor::FindPrevious,
            Some("EditorFind"),
        ),
        // View chrome
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-j", ToggleTerminal, None),
        KeyBinding::new("cmd-shift-e", ToggleEditor, None),
        KeyBinding::new("cmd-e", ToggleBrowser, None),
        KeyBinding::new("cmd-,", ToggleSettings, None),
        KeyBinding::new("cmd-=", IncreaseFontSize, None),
        KeyBinding::new("cmd-+", IncreaseFontSize, None),
        KeyBinding::new("cmd--", DecreaseFontSize, None),
        KeyBinding::new("cmd-0", ResetFontSize, None),
    ]);
}

pub fn init(cx: &mut App) {
    icons::load_icon_font(cx);
    bind_keys(cx);
}
