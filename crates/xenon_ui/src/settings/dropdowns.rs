//! Font family dropdowns and size steppers on the Fonts page.

use gpui::{App, Context, Entity, SharedString, Window};
use xenon_design_system::{TextInputEvent, TextInputView};

use super::SettingsView;
use crate::dropdown::{
    DropdownId, SizeTarget, filter_options, mono_font_families, ui_font_families,
};

impl SettingsView {
    pub(crate) fn toggle_dropdown(
        &mut self,
        id: DropdownId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open == Some(id) {
            self.close_dropdown(window, cx);
        } else {
            self.open = Some(id);
            self.opens_up = self
                .trigger_bounds
                .opens_up(id, window.viewport_size().height);
            self.filter.clear();
            self.highlight = 0;
            let current = crate::dropdown::current_label(id, cx);
            self.filter_input.update(cx, |input, cx| {
                input.set_placeholder(current, cx);
                input.set_text("", cx);
                input.open(cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn pick_dropdown(
        &mut self,
        id: DropdownId,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        apply_family(id, value, cx);
        self.close_dropdown(window, cx);
        cx.notify();
    }

    pub(crate) fn nudge_font_size(
        &mut self,
        target: SizeTarget,
        delta: f32,
        cx: &mut Context<Self>,
    ) {
        match target {
            SizeTarget::Ui => xenon_settings::nudge_ui_font_size(cx, delta),
            SizeTarget::Editor => xenon_settings::nudge_editor_font_size(cx, delta),
            SizeTarget::Terminal => xenon_settings::nudge_terminal_font_size(cx, delta),
        }
        xenon_settings::save(cx);
        xenon_terminal::refresh_windows(cx);
        cx.notify();
    }

    pub(crate) fn dismiss_dropdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.is_some() {
            self.close_dropdown(window, cx);
            cx.notify();
        }
    }

    fn close_dropdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self.filter.clear();
        self.highlight = 0;
        self.focus.focus(window, cx);
    }

    pub(super) fn on_filter_event(
        &mut self,
        _: &Entity<TextInputView>,
        event: &TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.open else {
            return;
        };
        match event {
            TextInputEvent::Changed(filter) => {
                self.filter = filter.clone();
                self.highlight = 0;
                cx.notify();
            }
            TextInputEvent::ParentKey { key, .. } => match key.as_str() {
                "up" => self.move_highlight(id, -1, cx),
                "down" => self.move_highlight(id, 1, cx),
                "enter" => {
                    if let Some(value) = self.filtered_options(id, cx).get(self.highlight).cloned()
                    {
                        self.pick_dropdown(id, value.to_string(), window, cx);
                    }
                }
                "escape" | "tab" => self.dismiss_dropdown(window, cx),
                _ => {}
            },
            TextInputEvent::Submit(_) | TextInputEvent::Cancel => {}
        }
    }

    fn move_highlight(&mut self, id: DropdownId, delta: isize, cx: &mut Context<Self>) {
        let len = self.filtered_options(id, cx).len();
        let next = self.highlight as isize + delta;
        self.highlight = next.clamp(0, len.saturating_sub(1) as isize) as usize;
        cx.notify();
    }

    fn filtered_options(&self, id: DropdownId, cx: &App) -> Vec<SharedString> {
        let options = match id {
            DropdownId::Ui => ui_font_families(cx),
            DropdownId::Editor | DropdownId::Terminal => mono_font_families(cx),
        };
        filter_options(&options, true, &self.filter)
    }
}

fn apply_family(id: DropdownId, value: String, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    match id {
        DropdownId::Ui => {
            settings.ui_font_family = xenon_settings::ensure_ui_family(&value, cx);
        }
        DropdownId::Editor => {
            settings.editor_font_family = xenon_settings::ensure_mono_family(&value, cx);
        }
        DropdownId::Terminal => {
            settings.terminal_font_family = xenon_settings::ensure_mono_family(&value, cx);
        }
    }
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::refresh_windows(cx);
}
