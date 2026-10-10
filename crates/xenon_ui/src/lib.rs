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
mod keymap;
mod palette;
mod rename;
mod resize;
mod settings;
mod sidebar;
mod tabs;
mod task_picker;
mod theme_picker;
mod toolbar;
mod toolbar_tooltip;
mod worklist_capture;
mod workspace_create;
mod workspace_discover;
mod workspace_picker;

pub use settings::SettingsView;
pub use visual_review::{disapproval_clipboard, selected_names};

mod visual_review;

#[cfg(feature = "visual-tests")]
pub use app::visual::{SCENES, Scene, apply_scene, surface_names};

use gpui::{App, actions};

pub use app::XenonApp;
pub(crate) use icons::preview_icon;
pub use xenon_settings::{Copy, CopyClean, CopyCode, Cut, Paste, SelectAll};

actions!(
    xenon,
    [
        NewTerminal,
        ToggleSidebar,
        ToggleTerminal,
        ToggleEditor,
        ToggleBrowser,
        SplitRight,
        SplitDown,
        ReserveEmptyPaneRight,
        RemoveEmptyPane,
        OpenFile,
        NewFile,
        NewFolder,
        AddWorkspace,
        NewWorkspace,
        FilePalette,
        RunTask,
        CloseEditor,
        CloseOtherTabs,
        CopyPath,
        CopyRelativePath,
        RevealInFinder,
        OpenInDefaultApp,
        Save,
        SaveAs,
        IncreaseFontSize,
        DecreaseFontSize,
        ResetFontSize,
        ToggleSettings,
        ToggleThemes,
        ToggleMobileRemote,
        ConnectPhone,
        ToggleKeepAwake,
        TogglePreview,
        ToggleSoftWrap,
        ToggleMemory,
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
        GoBack,
        GoForward,
        GoToDefinition,
        NextDiagnostic,
        PreviousDiagnostic,
        CommandPalette,
        KeyboardHelp,
        CaptureWorklist,
        OpenWorklist,
        UndoToast,
        DismissToast,
        OpenKeymap,
        Quit,
    ]
);

pub fn init(cx: &mut App) {
    xenon_settings::load_embedded_fonts(cx);
    icons::load_icon_font(cx);
    keymap::install(cx);
}
