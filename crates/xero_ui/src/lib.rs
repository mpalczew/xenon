//! The xero application shell: a collapsible workspace sidebar and a main panel
//! hosting a terminal and editor, with a cmd-p fuzzy finder.

mod app;
mod finder;
mod icons;
mod rename;
mod sidebar;
mod tabs;
mod toolbar;

use gpui::{App, KeyBinding, actions};

pub use app::XeroApp;
pub(crate) use icons::preview_icon;

actions!(
    xero,
    [
        ToggleSidebar,
        ToggleTerminal,
        ToggleEditor,
        ToggleBrowser,
        OpenFile,
        AddWorkspace,
        FilePalette,
        CloseEditor,
        IncreaseFontSize,
        DecreaseFontSize,
        ResetFontSize,
        ToggleSettings,
    ]
);

/// Bind the shell's keyboard shortcuts. Call once at startup.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-e", ToggleBrowser, None),
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-p", FilePalette, None),
        KeyBinding::new("cmd-w", CloseEditor, None),
        KeyBinding::new("cmd-shift-o", AddWorkspace, None),
        KeyBinding::new("cmd-=", IncreaseFontSize, None),
        KeyBinding::new("cmd-+", IncreaseFontSize, None),
        KeyBinding::new("cmd--", DecreaseFontSize, None),
        KeyBinding::new("cmd-0", ResetFontSize, None),
        KeyBinding::new("cmd-,", ToggleSettings, None),
    ]);
}

pub fn init(cx: &mut App) {
    icons::load_icon_font(cx);
    bind_keys(cx);
}
