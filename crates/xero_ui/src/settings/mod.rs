//! Settings as a dedicated window. Appearance + decoupled editor/terminal fonts.

mod input;
mod sections;

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div,
};
use theme::{ActiveTheme, FontFamilyCache};

use crate::ToggleSettings;
use crate::dropdown::{DropdownId, filter_options, font_size_options};
use input::input_registrar;
use sections::{OpenState, appearance_section, apply_dropdown_pick, editor_toggles, font_section};

pub struct SettingsView {
    focus: FocusHandle,
    open: Option<DropdownId>,
    filter: String,
    highlight: usize,
    focused_once: bool,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            open: None,
            filter: String::new(),
            highlight: 0,
            focused_once: false,
        }
    }

    pub(crate) fn toggle_dropdown(
        &mut self,
        id: DropdownId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open == Some(id) {
            self.close_dropdown();
        } else {
            self.open = Some(id);
            self.filter.clear();
            self.highlight = 0;
            self.focus.focus(window, cx);
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
        apply_dropdown_pick(id, value, cx);
        self.close_dropdown();
        window.refresh();
        cx.notify();
    }

    fn close_dropdown(&mut self) {
        self.open = None;
        self.filter.clear();
        self.highlight = 0;
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.open else {
            if event.keystroke.key.as_str() == "escape" {
                window.remove_window();
                cx.stop_propagation();
            }
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_dropdown();
                cx.notify();
                cx.stop_propagation();
            }
            "enter" => {
                if let Some(value) = self.filtered_options(id, cx).get(self.highlight).cloned() {
                    self.pick_dropdown(id, value.to_string(), window, cx);
                }
                cx.stop_propagation();
            }
            "up" => {
                self.move_highlight(id, -1, cx);
                cx.stop_propagation();
            }
            "down" => {
                self.move_highlight(id, 1, cx);
                cx.stop_propagation();
            }
            "backspace" if is_filterable(id) => {
                self.filter.pop();
                self.highlight = 0;
                cx.notify();
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn move_highlight(&mut self, id: DropdownId, delta: isize, cx: &mut Context<Self>) {
        let len = self.filtered_options(id, cx).len();
        if len == 0 {
            self.highlight = 0;
            return;
        }
        let next = self.highlight as isize + delta;
        self.highlight = next.clamp(0, (len - 1) as isize) as usize;
        cx.notify();
    }

    fn filtered_options(&self, id: DropdownId, cx: &App) -> Vec<SharedString> {
        filter_options(&options_for(id, cx), is_filterable(id), &self.filter)
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = xero_settings::snapshot(cx);
        let families = FontFamilyCache::global(cx).list_font_families(cx);
        let sizes = font_size_options();
        let state = OpenState {
            open: self.open,
            filter: self.filter.as_str(),
            highlight: self.highlight,
        };
        div()
            .id("settings-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(appearance_section(&settings, state, cx))
            .child(font_section(
                "Editor Font",
                DropdownId::EditorFamily,
                DropdownId::EditorSize,
                &settings.editor_font_family,
                settings.editor_font_size,
                &families,
                &sizes,
                state,
                cx,
            ))
            .child(font_section(
                "Terminal Font",
                DropdownId::TerminalFamily,
                DropdownId::TerminalSize,
                &settings.terminal_font_family,
                settings.terminal_font_size,
                &families,
                &sizes,
                state,
                cx,
            ))
            .child(editor_toggles(cx))
            .into_any_element()
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title("Settings");
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let body = self.body(cx);
        div()
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_action(cx.listener(|_, _: &ToggleSettings, window, _cx| {
                window.remove_window();
            }))
            .on_key_down(cx.listener(Self::on_key))
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(div().text_lg().child("Settings")),
            )
            .child(body)
            .child(input_registrar(cx.entity(), self.focus.clone()))
    }
}

fn is_filterable(id: DropdownId) -> bool {
    !matches!(id, DropdownId::Mode)
}

fn options_for(id: DropdownId, cx: &App) -> Vec<SharedString> {
    match id {
        DropdownId::Mode => vec!["System".into(), "Light".into(), "Dark".into()],
        DropdownId::LightTheme => xero_terminal::theme_names(theme::Appearance::Light, cx),
        DropdownId::DarkTheme => xero_terminal::theme_names(theme::Appearance::Dark, cx),
        DropdownId::EditorFamily | DropdownId::TerminalFamily => {
            FontFamilyCache::global(cx).list_font_families(cx)
        }
        DropdownId::EditorSize | DropdownId::TerminalSize => font_size_options(),
    }
}
