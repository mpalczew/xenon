//! Settings window open/close (deferred so cmd-, is safe on the key path).

use super::*;
use gpui::{TitlebarOptions, WindowBounds, WindowOptions, size};

impl XenonApp {
    /// Open the settings window, or focus/close it if already open (cmd-,).
    ///
    /// Opening is deferred one tick so it does not run inside AppKit's
    /// `performKeyEquivalent` stack (panics there become process aborts).
    pub(crate) fn toggle_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            match handle.is_active(cx) {
                Some(true) => {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    self.settings_window = None;
                    return;
                }
                Some(false) => {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                    return;
                }
                None => self.settings_window = None,
            }
        }
        cx.spawn(async move |this, cx| {
            this.update(cx, |this, cx| this.open_settings_window_now(cx))
                .ok();
        })
        .detach();
    }

    fn open_settings_window_now(&mut self, cx: &mut Context<Self>) {
        if self.settings_window.is_some() {
            return;
        }
        let bounds = Bounds::centered(None, size(px(640.), px(480.)), cx);
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Settings".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                xenon_terminal::observe_appearance(window, cx).detach();
                cx.new(SettingsView::new)
            },
        ) {
            Ok(handle) => self.settings_window = Some(handle),
            Err(error) => log::error!("failed to open settings window: {error}"),
        }
    }
}
