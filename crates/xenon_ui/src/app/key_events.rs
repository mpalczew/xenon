//! Root keyboard routing and shortcut diagnostics.

use super::*;

impl XenonApp {
    pub(super) fn on_app_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.on_skill_prompt_key(event, cx)
            || self.on_tab_menu_key(event, window, cx)
            || self.on_overflow_menu_key(event, window, cx)
            || self.on_browser_menu_key(event, window, cx)
            || self.on_workspace_menu_key(event, window, cx)
            || self.on_browser_key(event, window, cx)
        {
            cx.stop_propagation();
        }
    }

    pub(super) fn log_shortcut_key_event(
        &self,
        event: &gpui::KeyDownEvent,
        window: &Window,
        cx: &Context<Self>,
    ) {
        let key = event.keystroke.key.to_ascii_lowercase();
        if event.keystroke.modifiers.platform && matches!(key.as_str(), "n" | "o") {
            log::info!(
                "shortcut key event: {:?}; window_active={}; gpui_focus={:?}; xenon_focus={:?}",
                event.keystroke,
                window.is_window_active(),
                window.focused(cx),
                self.current_focus_owner(window, cx),
            );
        }
    }

    pub(super) fn log_shortcut_action(&self, name: &str, window: &Window, cx: &Context<Self>) {
        log::info!(
            "shortcut action: {name}; window_active={}; gpui_focus={:?}; xenon_focus={:?}",
            window.is_window_active(),
            window.focused(cx),
            self.current_focus_owner(window, cx),
        );
    }
}
