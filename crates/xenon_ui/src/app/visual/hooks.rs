use super::{Scene, XenonApp};
use crate::settings::SettingsView;
use gpui::{Context, WindowHandle};

impl XenonApp {
    /// Runner hook after the scene's terminal is ready (for PTY-dependent state).
    pub fn visual_after_terminal_ready(&mut self, scene: Scene, cx: &mut Context<XenonApp>) {
        if scene == Scene::PhoneDriving {
            self.visual_phone_driving(cx);
        }
    }

    pub fn visual_settings_window(&self) -> Option<WindowHandle<SettingsView>> {
        self.settings_window
    }

    pub fn visual_terminal_ready(&self, cx: &gpui::App) -> bool {
        self.active_terminal()
            .is_some_and(|view| view.read(cx).visual_is_ready())
    }
}
