//! Theme side panel wiring (⌘⌥T). The panel previews live, so closing it any
//! way other than Keep must put the saved theme back.

use super::*;
use crate::theme_picker::{ThemePickerEvent, ThemePickerView};
use theme::Appearance;

impl XenonApp {
    pub(super) fn dismiss_palettes(&mut self, cx: &mut Context<Self>) {
        self.finder = None;
        self.task_picker = None;
        self.workspace_picker = None;
        self.workspace_create = None;
        self.command_palette = None;
        self.close_theme_picker(cx);
    }

    /// Drop the panel and undo any preview it left on screen.
    pub(super) fn close_theme_picker(&mut self, cx: &mut Context<Self>) {
        if self.theme_picker.take().is_some() {
            xenon_terminal::apply_theme(cx);
        }
    }

    /// ⌘⌥T opens the panel, or reverts and closes it when already open.
    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_picker.is_some() {
            self.close_theme_picker(cx);
            self.restore_focus_after_theme_picker(cx);
            return;
        }
        self.dismiss_palettes(cx);
        self.browser_focused = false;
        self.deferred.restore_pane = Some(self.current_focus_owner(window, cx));
        let picker = cx.new(ThemePickerView::new);
        self._theme_picker_sub = Some(cx.subscribe(&picker, Self::on_theme_picker_event));
        self.theme_picker = Some(picker);
        cx.notify();
    }

    fn on_theme_picker_event(
        &mut self,
        _picker: Entity<ThemePickerView>,
        event: &ThemePickerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ThemePickerEvent::Dismissed => {
                // The panel already kept or reverted; do not revert again.
                self.theme_picker = None;
                self.restore_focus_after_theme_picker(cx);
            }
        }
    }

    fn restore_focus_after_theme_picker(&mut self, cx: &mut Context<Self>) {
        self.deferred.pending_focus = self
            .deferred
            .restore_pane
            .take()
            .or_else(|| Some(self.fallback_content_pane()));
        cx.notify();
    }

    /// Docked on the right under the toolbar; the rest of the app stays visible.
    pub(super) fn render_theme_panel(&self) -> Option<impl IntoElement + use<>> {
        let panel = self.theme_picker.clone()?;
        Some(
            div()
                .absolute()
                .top(px(crate::toolbar::TOOLBAR_HEIGHT))
                .right_0()
                .bottom_0()
                .w(px(crate::theme_picker::PANEL_WIDTH))
                .child(panel),
        )
    }
}

/// Store a theme in its light/dark slot. Repaint only when that slot is on screen.
pub(crate) fn apply_named_theme(name: &str, appearance: Appearance, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    match appearance {
        Appearance::Light => settings.light_theme = name.to_string(),
        Appearance::Dark => settings.dark_theme = name.to_string(),
    }
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::apply_theme(cx);
}
