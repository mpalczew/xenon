//! Theme gallery overlay wiring (⌘⌥T).

use super::*;
use crate::theme_picker::{ThemePickerEvent, ThemePickerView};

impl XenonApp {
    pub(super) fn dismiss_palettes(&mut self) {
        self.finder = None;
        self.task_picker = None;
        self.workspace_picker = None;
        self.command_palette = None;
        self.theme_picker = None;
    }

    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_palettes();
        self.browser_focused = false;
        self.deferred.restore_pane = self.focused_pane(window, cx);
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
                self.theme_picker = None;
                self.deferred.pending_focus = self
                    .deferred
                    .restore_pane
                    .take()
                    .or_else(|| Some(self.fallback_content_pane()));
                cx.notify();
            }
        }
    }
}
