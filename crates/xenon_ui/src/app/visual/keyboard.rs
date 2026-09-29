//! Hooks for the visual runner's keyboard regressions.

use gpui::{FocusHandle, Focusable as _, Window};

use super::XenonApp;

impl XenonApp {
    /// Put keys on the active editor, as a user landing on the worklist tab.
    pub fn visual_focus_editor(&self, window: &mut Window, cx: &mut gpui::App) {
        if let Some(editor) = self.active_editor() {
            window.focus(&editor.focus_handle(cx), cx);
        }
    }

    /// Focus a live handle that no element renders. The caller keeps it
    /// alive; dropping it would be the other (dead handle) case.
    pub fn visual_focus_offscreen(&self, window: &mut Window, cx: &mut gpui::App) -> FocusHandle {
        let handle = cx.focus_handle();
        window.focus(&handle, cx);
        handle
    }

    /// Keep only the keystroke fallback, to test it without the focus-lost
    /// listener repairing first.
    pub fn visual_keystroke_guard_only(&mut self) {
        drop(self.services.focus_guard.remove(0));
    }

    pub fn visual_command_palette_open(&self) -> bool {
        self.command_palette.is_some()
    }
}
