//! Settings as a dedicated window. Appearance + decoupled editor/terminal fonts.

mod remote_edit;
mod remote_section;
mod sections;
mod skill_section;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window, div,
};
use theme::ActiveTheme;
use xenon_design_system::{FocusOnOpen, TextInputConfig, TextInputEvent, TextInputView};

use crate::ToggleSettings;
use crate::dropdown::{
    DropdownId, SizeTarget, filter_options, mono_font_families, ui_font_families,
};
use remote_section::remote_section;
use sections::{
    OpenState, appearance_section, apply_dropdown_pick, apply_size_nudge, editor_toggles,
    font_section, terminal_section,
};
use skill_section::skill_section;

use remote_edit::RemoteFieldEdit;

pub struct SettingsView {
    focus: FocusHandle,
    focus_on_open: FocusOnOpen,
    open: Option<DropdownId>,
    filter: String,
    filter_input: Entity<TextInputView>,
    _filter_sub: Subscription,
    highlight: usize,
    /// Keyboard highlight among toggles: 0 line numbers, 1 vim, 2 mobile remote.
    toggle_focus: usize,
    /// Inline edit for remote password/hostname (None = not editing).
    remote_edit: Option<RemoteFieldEdit>,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let filter_input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line("Type to filter…").parent_navigation(),
                cx,
            )
        });
        let filter_sub = cx.subscribe(&filter_input, |this, _, event, cx| {
            if let TextInputEvent::Changed(filter) = event {
                this.filter = filter.clone();
                this.highlight = 0;
                cx.notify();
            }
        });
        // Full system font scans are deferred and never run on paint / key path.
        cx.spawn(async move |this, cx| {
            this.update(cx, |_this, cx| {
                crate::dropdown::warm_mono_font_families(cx);
                crate::dropdown::warm_ui_font_families(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        let focus = cx.focus_handle();
        let mut focus_on_open = FocusOnOpen::new(focus.clone());
        focus_on_open.open();
        Self {
            focus,
            focus_on_open,
            open: None,
            filter: String::new(),
            filter_input,
            _filter_sub: filter_sub,
            highlight: 0,
            toggle_focus: 0,
            remote_edit: None,
        }
    }

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
            self.filter.clear();
            self.highlight = 0;
            if is_filterable(id) {
                self.filter_input.update(cx, |input, cx| {
                    input.set_text("", cx);
                    input.open(cx);
                });
            } else {
                self.focus.focus(window, cx);
            }
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
        self.close_dropdown(window, cx);
        cx.notify();
    }

    pub(crate) fn nudge_font_size(
        &mut self,
        target: SizeTarget,
        delta: f32,
        cx: &mut Context<Self>,
    ) {
        apply_size_nudge(target, delta, cx);
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

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_remote_edit_key(event, window, cx) {
            return;
        }
        let Some(id) = self.open else {
            match event.keystroke.key.as_str() {
                "escape" => {
                    window.remove_window();
                    cx.stop_propagation();
                }
                "up" => {
                    self.toggle_focus = self.toggle_focus.saturating_sub(1);
                    cx.notify();
                    cx.stop_propagation();
                }
                "down" => {
                    self.toggle_focus = (self.toggle_focus + 1).min(3);
                    cx.notify();
                    cx.stop_propagation();
                }
                "enter" | " " => {
                    self.activate_focused_toggle(window, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_dropdown(window, cx);
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
            _ => {}
        }
    }

    fn activate_focused_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.toggle_focus {
            0 => {
                xenon_settings::toggle_line_numbers(cx);
                xenon_settings::save(cx);
            }
            1 => {
                xenon_settings::toggle_vim_mode(cx);
                xenon_settings::save(cx);
            }
            2 => remote_section::activate_mobile_remote(window, cx),
            _ => skill_section::toggle_skill_from_keys(cx),
        }
        window.refresh();
        cx.notify();
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

    fn body(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = xenon_settings::snapshot(cx);
        // Cache only / seed list — never full system scan during paint.
        let mono = mono_font_families(cx);
        let ui = ui_font_families(cx);
        let state = OpenState {
            open: self.open,
            filter: self.filter.as_str(),
            filter_input: &self.filter_input,
            highlight: self.highlight,
            viewport_height: window.viewport_size().height,
        };
        div()
            .id("settings-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_dropdown(window, cx);
            }))
            .child(appearance_section(&settings, cx))
            .child(skill_section(self.toggle_focus == 3, cx))
            .child(font_section(
                "UI Font",
                DropdownId::UiFamily,
                SizeTarget::Ui,
                &settings.ui_font_family,
                settings.ui_font_size,
                &ui,
                state,
                cx,
            ))
            .child(font_section(
                "Editor Font",
                DropdownId::EditorFamily,
                SizeTarget::Editor,
                &settings.editor_font_family,
                settings.editor_font_size,
                &mono,
                state,
                cx,
            ))
            .child(font_section(
                "Terminal Font",
                DropdownId::TerminalFamily,
                SizeTarget::Terminal,
                &settings.terminal_font_family,
                settings.terminal_font_size,
                &mono,
                state,
                cx,
            ))
            .child(editor_toggles(self.toggle_focus, cx))
            .child(terminal_section(&settings, state, cx))
            .child(remote_section(
                &crate::app::remote::mobile_remote_info(cx),
                self.toggle_focus == 2,
                self.remote_edit
                    .as_ref()
                    .map(|re| (re.field, re.input.clone())),
                cx,
            ))
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
        self.focus_on_open.focus_after_open(window, cx);
        let ui = xenon_settings::ui_font(cx);
        window.set_rem_size(gpui::px(ui.size));
        let colors = cx.theme().colors().clone();
        let body = self.body(window, cx);
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
            .font_family(ui.family)
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(div().text_lg().child("Settings")),
            )
            .child(body)
    }
}

pub(super) fn is_filterable(id: DropdownId) -> bool {
    !matches!(id, DropdownId::TerminalAutoClose)
}

fn options_for(id: DropdownId, cx: &App) -> Vec<SharedString> {
    match id {
        DropdownId::UiFamily => ui_font_families(cx),
        DropdownId::EditorFamily | DropdownId::TerminalFamily => mono_font_families(cx),
        DropdownId::TerminalAutoClose => [
            xenon_settings::TerminalAutoClose::Off,
            xenon_settings::TerminalAutoClose::Immediate,
            xenon_settings::TerminalAutoClose::After1s,
            xenon_settings::TerminalAutoClose::After3s,
            xenon_settings::TerminalAutoClose::After5s,
        ]
        .into_iter()
        .map(|m| SharedString::from(m.label()))
        .collect(),
    }
}
