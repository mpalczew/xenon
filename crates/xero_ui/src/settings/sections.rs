//! Settings window section builders (appearance, fonts, editor toggles).

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;

use super::SettingsView;
use crate::dropdown::{DropdownId, DropdownProps, dropdown_row, format_size};

#[derive(Clone, Copy)]
pub(super) struct OpenState<'a> {
    pub open: Option<DropdownId>,
    pub filter: &'a str,
    pub highlight: usize,
}

pub(super) fn appearance_section(
    settings: &xero_store::AppSettings,
    state: OpenState<'_>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let mode_opts: Vec<SharedString> = vec!["System".into(), "Light".into(), "Dark".into()];
    let light = xero_terminal::theme_names(theme::Appearance::Light, cx);
    let dark = xero_terminal::theme_names(theme::Appearance::Dark, cx);
    div()
        .flex()
        .flex_col()
        .child(section_header("Appearance", cx))
        .child(dropdown_row(
            DropdownProps {
                id: DropdownId::Mode,
                title: "Mode",
                subtitle: "Light, dark, or match the system",
                selected: mode_label(settings.theme),
                options: &mode_opts,
                filterable: false,
                open: state.open == Some(DropdownId::Mode),
                filter: state.filter,
                highlight: state.highlight,
            },
            cx,
        ))
        .child(dropdown_row(
            DropdownProps {
                id: DropdownId::LightTheme,
                title: "Light Theme",
                subtitle: "When mode is Light, or System is light",
                selected: &settings.light_theme,
                options: &light,
                filterable: true,
                open: state.open == Some(DropdownId::LightTheme),
                filter: state.filter,
                highlight: state.highlight,
            },
            cx,
        ))
        .child(dropdown_row(
            DropdownProps {
                id: DropdownId::DarkTheme,
                title: "Dark Theme",
                subtitle: "When mode is Dark, or System is dark",
                selected: &settings.dark_theme,
                options: &dark,
                filterable: true,
                open: state.open == Some(DropdownId::DarkTheme),
                filter: state.filter,
                highlight: state.highlight,
            },
            cx,
        ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn font_section(
    title: &'static str,
    family_id: DropdownId,
    size_id: DropdownId,
    family: &str,
    size: f32,
    families: &[SharedString],
    sizes: &[SharedString],
    state: OpenState<'_>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let size_label = format_size(size);
    div()
        .flex()
        .flex_col()
        .child(section_header(title, cx))
        .child(dropdown_row(
            DropdownProps {
                id: family_id,
                title: "Font Family",
                subtitle: "Typeface for this surface",
                selected: family,
                options: families,
                filterable: true,
                open: state.open == Some(family_id),
                filter: state.filter,
                highlight: state.highlight,
            },
            cx,
        ))
        .child(dropdown_row(
            DropdownProps {
                id: size_id,
                title: "Font Size",
                subtitle: "Point size for this surface",
                selected: &size_label,
                options: sizes,
                filterable: true,
                open: state.open == Some(size_id),
                filter: state.filter,
                highlight: state.highlight,
            },
            cx,
        ))
}

pub(super) fn editor_toggles(cx: &mut Context<SettingsView>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .child(section_header("Editor", cx))
        .child(settings_toggle(
            ToggleRow {
                id: "line-number-toggle",
                title: "Line numbers",
                subtitle: "Show row numbers in text editors",
                checked: xero_settings::show_line_numbers(cx),
            },
            cx,
            |cx| {
                xero_settings::toggle_line_numbers(cx);
                xero_settings::save(cx);
            },
        ))
        .child(settings_toggle(
            ToggleRow {
                id: "vim-mode-toggle",
                title: "Vim mode",
                subtitle: "Modal editing in the text editor",
                checked: xero_settings::vim_mode(cx),
            },
            cx,
            |cx| {
                xero_settings::toggle_vim_mode(cx);
                xero_settings::save(cx);
            },
        ))
}

pub(super) fn apply_dropdown_pick(id: DropdownId, value: String, cx: &mut App) {
    let mut settings = xero_settings::snapshot(cx);
    match id {
        DropdownId::Mode => settings.theme = parse_mode(&value),
        DropdownId::LightTheme => settings.light_theme = value,
        DropdownId::DarkTheme => settings.dark_theme = value,
        DropdownId::EditorFamily => settings.editor_font_family = value,
        DropdownId::EditorSize => {
            if let Ok(size) = value.parse::<f32>() {
                settings.editor_font_size = size;
            }
        }
        DropdownId::TerminalFamily => settings.terminal_font_family = value,
        DropdownId::TerminalSize => {
            if let Ok(size) = value.parse::<f32>() {
                settings.terminal_font_size = size;
            }
        }
    }
    xero_settings::apply(&settings, cx);
    xero_settings::save(cx);
    if matches!(
        id,
        DropdownId::Mode | DropdownId::LightTheme | DropdownId::DarkTheme
    ) {
        xero_terminal::apply_theme(cx);
    }
}

fn section_header(title: &'static str, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .px_4()
        .pt_4()
        .pb_1()
        .text_sm()
        .text_color(colors.text_muted)
        .child(title)
}

struct ToggleRow {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    checked: bool,
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
    div()
        .id(row.id)
        .flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(colors.border)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
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
        .on_click(cx.listener(move |_, _, window, cx| {
            on_toggle(cx);
            window.refresh();
            cx.notify();
        }))
}

fn mode_label(mode: xero_settings::ThemeMode) -> &'static str {
    match mode {
        xero_settings::ThemeMode::System => "System",
        xero_settings::ThemeMode::Light => "Light",
        xero_settings::ThemeMode::Dark => "Dark",
    }
}

fn parse_mode(label: &str) -> xero_settings::ThemeMode {
    match label {
        "Light" => xero_settings::ThemeMode::Light,
        "Dark" => xero_settings::ThemeMode::Dark,
        _ => xero_settings::ThemeMode::System,
    }
}
