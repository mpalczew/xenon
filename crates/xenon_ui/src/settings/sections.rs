//! Settings window section builders (appearance, fonts, editor toggles).

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;

use super::SettingsView;
use crate::dropdown::{DropdownId, DropdownProps, SizeTarget, dropdown_row, size_row};

#[derive(Clone, Copy)]
pub(super) struct OpenState<'a> {
    pub open: Option<DropdownId>,
    pub filter: &'a str,
    pub highlight: usize,
    pub caret_on: bool,
    pub viewport_height: Pixels,
}

pub(super) fn appearance_section(
    settings: &xenon_store::AppSettings,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let body = div()
        .flex()
        .flex_col()
        .child(mode_segments(settings.theme, cx))
        .child(row_divider(cx))
        .child(current_theme_row(settings, cx));
    group_card("Appearance", body, cx)
}

fn mode_segments(
    selected: xenon_settings::ThemeMode,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    use xenon_settings::ThemeMode;
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .child(div().text_sm().text_color(colors.text_muted).child("Mode"))
        .child(
            div()
                .flex()
                .gap_1()
                .child(mode_seg("System", ThemeMode::System, selected, cx))
                .child(mode_seg("Light", ThemeMode::Light, selected, cx))
                .child(mode_seg("Dark", ThemeMode::Dark, selected, cx)),
        )
}

fn mode_seg(
    label: &'static str,
    mode: xenon_settings::ThemeMode,
    selected: xenon_settings::ThemeMode,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let on = mode == selected;
    div()
        .id(label)
        .flex_1()
        .px_2()
        .py_1()
        .rounded_sm()
        .flex()
        .items_center()
        .justify_center()
        .bg(if on {
            colors.element_selected
        } else {
            gpui::transparent_black()
        })
        .text_sm()
        .font_weight(if on {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if on { colors.text } else { colors.text_muted })
        .cursor_pointer()
        .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
        .on_click(cx.listener(move |_, _, window, cx| {
            cx.stop_propagation();
            apply_mode(mode, cx);
            window.refresh();
            cx.notify();
        }))
        .child(label)
}

fn current_theme_row(
    settings: &xenon_store::AppSettings,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let name = live_theme_name(settings, cx);
    div()
        .id("current-theme")
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .on_click(cx.listener(|_, _, window, cx| {
            cx.stop_propagation();
            open_theme_gallery(window, cx);
        }))
        .child(div().text_sm().text_color(colors.text_muted).child("Theme"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_sm().child(name))
                .child(div().text_xs().text_color(colors.text_muted).child("⌘⌥T")),
        )
}

fn live_theme_name(settings: &xenon_store::AppSettings, cx: &App) -> String {
    match showing_appearance(settings.theme, cx) {
        theme::Appearance::Light => settings.light_theme.clone(),
        theme::Appearance::Dark => settings.dark_theme.clone(),
    }
}

fn showing_appearance(mode: xenon_settings::ThemeMode, cx: &App) -> theme::Appearance {
    match mode {
        xenon_settings::ThemeMode::Light => theme::Appearance::Light,
        xenon_settings::ThemeMode::Dark => theme::Appearance::Dark,
        xenon_settings::ThemeMode::System => theme::SystemAppearance::global(cx).0,
    }
}

fn apply_mode(mode: xenon_settings::ThemeMode, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    settings.theme = mode;
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::apply_theme(cx);
}

fn open_theme_gallery(_window: &mut Window, cx: &mut App) {
    for handle in cx.windows() {
        let Some(main) = handle.downcast::<crate::XenonApp>() else {
            continue;
        };
        let _ = main.update(cx, |app, window, cx| {
            app.open_theme_picker(window, cx);
            window.activate_window();
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn font_section(
    title: &'static str,
    family_id: DropdownId,
    size_target: SizeTarget,
    family: &str,
    size: f32,
    families: &[SharedString],
    state: OpenState<'_>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let body = div()
        .flex()
        .flex_col()
        .child(dropdown_row(
            DropdownProps {
                id: family_id,
                title: "Family",
                selected: family,
                options: families,
                filterable: true,
                open: state.open == Some(family_id),
                filter: state.filter,
                highlight: state.highlight,
                caret_on: state.caret_on,
                viewport_height: state.viewport_height,
            },
            cx,
        ))
        .child(row_divider(cx))
        .child(size_row("Size", size_target, size, cx));
    group_card(title, body, cx)
}

pub(super) fn editor_toggles(
    toggle_focus: usize,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    // Caller passes focus index — never read SettingsView during its own render
    // (double-borrow → panic_cannot_unwind on the cmd-, key path).
    let body = div()
        .flex()
        .flex_col()
        .child(settings_toggle(
            ToggleRow {
                id: "line-number-toggle",
                title: "Line numbers",
                subtitle: "Show row numbers in text editors",
                checked: xenon_settings::show_line_numbers(cx),
                focused: toggle_focus == 0,
            },
            cx,
            |cx| {
                xenon_settings::toggle_line_numbers(cx);
                xenon_settings::save(cx);
            },
        ))
        .child(row_divider(cx))
        .child(settings_toggle(
            ToggleRow {
                id: "vim-mode-toggle",
                title: "Vim mode",
                subtitle: "Modal editing in the text editor",
                checked: xenon_settings::vim_mode(cx),
                focused: toggle_focus == 1,
            },
            cx,
            |cx| {
                xenon_settings::toggle_vim_mode(cx);
                xenon_settings::save(cx);
            },
        ));
    group_card("Editor", body, cx)
}

pub(super) fn terminal_section(
    settings: &xenon_store::AppSettings,
    state: OpenState<'_>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let opts: Vec<SharedString> = auto_close_options()
        .iter()
        .map(|m| SharedString::from(m.label()))
        .collect();
    let body = div().flex().flex_col().child(dropdown_row(
        DropdownProps {
            id: DropdownId::TerminalAutoClose,
            title: "Default close when process exits",
            selected: settings.terminal_auto_close.label(),
            options: &opts,
            filterable: false,
            open: state.open == Some(DropdownId::TerminalAutoClose),
            filter: state.filter,
            highlight: state.highlight,
            caret_on: state.caret_on,
            viewport_height: state.viewport_height,
        },
        cx,
    ));
    group_card("Terminal", body, cx)
}

/// Mobile PTY remote: enable toggle + live URL/token (not persisted).
fn auto_close_options() -> [xenon_settings::TerminalAutoClose; 5] {
    use xenon_settings::TerminalAutoClose::*;
    [Off, Immediate, After1s, After3s, After5s]
}

pub(super) fn apply_dropdown_pick(id: DropdownId, value: String, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    match id {
        DropdownId::UiFamily => {
            settings.ui_font_family = xenon_settings::ensure_ui_family(&value, cx);
        }
        DropdownId::EditorFamily => {
            settings.editor_font_family = xenon_settings::ensure_mono_family(&value, cx);
        }
        DropdownId::TerminalFamily => {
            settings.terminal_font_family = xenon_settings::ensure_mono_family(&value, cx);
        }
        DropdownId::TerminalAutoClose => {
            settings.terminal_auto_close = parse_auto_close(&value);
        }
    }
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::refresh_windows(cx);
}

pub(super) fn apply_size_nudge(target: SizeTarget, delta: f32, cx: &mut App) {
    match target {
        SizeTarget::Ui => xenon_settings::nudge_ui_font_size(cx, delta),
        SizeTarget::Editor => xenon_settings::nudge_editor_font_size(cx, delta),
        SizeTarget::Terminal => xenon_settings::nudge_terminal_font_size(cx, delta),
    }
    xenon_settings::save(cx);
    xenon_terminal::refresh_windows(cx);
}

pub(super) fn group_card(
    title: &'static str,
    body: impl IntoElement,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .flex_col()
        .px_4()
        .pt_3()
        .pb_1()
        .child(
            div()
                .text_sm()
                .text_color(colors.text_muted)
                .mb_1()
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .bg(colors.elevated_surface_background)
                .child(body),
        )
}

pub(super) fn row_divider(cx: &mut Context<SettingsView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div().h(px(1.)).bg(colors.border)
}

struct ToggleRow {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    checked: bool,
    focused: bool,
}

fn settings_toggle(
    row: ToggleRow,
    cx: &mut Context<SettingsView>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let background = if row.checked {
        colors.element_selected
    } else {
        colors.elevated_surface_background
    };
    let row_bg = if row.focused {
        colors.element_hover
    } else {
        gpui::transparent_black()
    };
    div()
        .id(row.id)
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .bg(row_bg)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .on_click(cx.listener(move |_, _, window, cx| {
            cx.stop_propagation();
            on_toggle(cx);
            window.refresh();
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_sm().child(row.title))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(row.subtitle),
                ),
        )
        .child(
            div()
                .w(px(18.))
                .h(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .bg(background)
                .text_xs()
                .children(row.checked.then_some("x")),
        )
}

fn parse_auto_close(label: &str) -> xenon_settings::TerminalAutoClose {
    auto_close_options()
        .into_iter()
        .find(|m| m.label() == label)
        .unwrap_or_default()
}
