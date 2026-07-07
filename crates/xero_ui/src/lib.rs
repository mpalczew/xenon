//! The xero application shell: a collapsible workspace sidebar and a main panel
//! hosting a terminal and editor, with a cmd-p fuzzy finder.

mod app;
mod finder;
mod sidebar;
mod toolbar;

use gpui::{App, KeyBinding, actions};

pub use app::XeroApp;

actions!(xero, [ToggleSidebar, OpenFile, AddWorkspace, FilePalette, CloseEditor]);

/// Bind the shell's keyboard shortcuts. Call once at startup.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-p", FilePalette, None),
        KeyBinding::new("cmd-w", CloseEditor, None),
        KeyBinding::new("cmd-shift-o", AddWorkspace, None),
    ]);
}
