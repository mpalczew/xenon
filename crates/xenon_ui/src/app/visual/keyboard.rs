//! Hooks for the visual runner's keyboard regressions.

use gpui::{Focusable as _, Window};

use super::XenonApp;

impl XenonApp {
    /// Put keys on the active editor, as a user landing on the worklist tab.
    pub fn visual_focus_editor(&self, window: &mut Window, cx: &mut gpui::App) {
        if let Some(editor) = self.active_editor() {
            window.focus(&editor.focus_handle(cx), cx);
        }
    }

    pub fn visual_command_palette_open(&self) -> bool {
        self.command_palette.is_some()
    }
}
