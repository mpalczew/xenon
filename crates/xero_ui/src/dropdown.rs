//! Filterable / plain dropdown used by the Settings window.

use gpui::{
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, px,
};
use theme::ActiveTheme;

use crate::settings::SettingsView;

/// Which settings dropdown is open (at most one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropdownId {
    Mode,
    LightTheme,
    DarkTheme,
    EditorFamily,
    EditorSize,
    TerminalFamily,
    TerminalSize,
}

pub(crate) struct DropdownProps<'a> {
    pub id: DropdownId,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub selected: &'a str,
    pub options: &'a [SharedString],
    pub filterable: bool,
    pub open: bool,
    pub filter: &'a str,
    pub highlight: usize,
}

struct ListProps<'a> {
    id: DropdownId,
    options: &'a [SharedString],
    selected: &'a str,
    filterable: bool,
    filter: &'a str,
    highlight: usize,
}

pub(crate) fn dropdown_row(
    props: DropdownProps<'_>,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let filtered = filter_options(props.options, props.filterable, props.filter);
    let mut row = div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(colors.border)
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w_0()
                        .flex_1()
                        .child(div().text_sm().child(props.title))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(props.subtitle),
                        ),
                )
                .child(trigger(props.id, props.selected, props.open, cx)),
        );
    if props.open {
        row = row.child(option_list(
            ListProps {
                id: props.id,
                options: &filtered,
                selected: props.selected,
                filterable: props.filterable,
                filter: props.filter,
                highlight: props.highlight,
            },
            cx,
        ));
    }
    row
}

fn trigger(
    id: DropdownId,
    selected: &str,
    open: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let label = selected.to_string();
    let chevron = if open { "▴" } else { "▾" };
    div()
        .id(SharedString::from(format!("dd-trigger-{id:?}")))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .w(px(200.))
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .text_xs()
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(label),
        )
        .child(div().text_color(colors.text_muted).child(chevron))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.toggle_dropdown(id, window, cx);
        }))
}

fn option_list(props: ListProps<'_>, cx: &mut gpui::Context<SettingsView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let mut list = div()
        .id(SharedString::from(format!("dd-list-{:?}", props.id)))
        .flex()
        .flex_col()
        .mt_1()
        .max_h(px(220.))
        .overflow_y_scroll()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background);

    if props.filterable {
        list = list.child(filter_banner(props.filter, cx));
    }
    if props.options.is_empty() {
        list = list.child(
            div()
                .px_2()
                .py_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child("No matches"),
        );
    } else {
        for (index, option) in props.options.iter().enumerate() {
            list = list.child(option_row(
                props.id,
                option.clone(),
                option.as_ref() == props.selected,
                index == props.highlight,
                cx,
            ));
        }
    }
    list
}

fn filter_banner(filter: &str, cx: &mut gpui::Context<SettingsView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let empty = filter.is_empty();
    let label = if empty {
        "Type to filter…".to_string()
    } else {
        filter.to_string()
    };
    div()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(colors.border)
        .text_xs()
        .text_color(if empty {
            colors.text_muted
        } else {
            colors.text
        })
        .child(label)
}

fn option_row(
    id: DropdownId,
    option: SharedString,
    selected: bool,
    highlighted: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let background = if highlighted {
        colors.element_selected
    } else if selected {
        colors.element_hover
    } else {
        colors.elevated_surface_background
    };
    let pick = option.clone();
    div()
        .id(SharedString::from(format!("dd-opt-{id:?}-{option}")))
        .px_2()
        .py_1()
        .text_xs()
        .bg(background)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .child(option)
        .on_click(cx.listener(move |this, _, window, cx| {
            this.pick_dropdown(id, pick.to_string(), window, cx);
        }))
}

pub(crate) fn filter_options(
    options: &[SharedString],
    filterable: bool,
    filter: &str,
) -> Vec<SharedString> {
    if !filterable || filter.is_empty() {
        return options.to_vec();
    }
    let needle = filter.to_lowercase();
    options
        .iter()
        .filter(|name| name.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Font size options from MIN..=MAX as display strings.
pub(crate) fn font_size_options() -> Vec<SharedString> {
    let min = xero_settings::MIN_FONT_SIZE as i32;
    let max = xero_settings::MAX_FONT_SIZE as i32;
    (min..=max)
        .map(|n| SharedString::from(n.to_string()))
        .collect()
}

pub(crate) fn format_size(size: f32) -> String {
    if (size - size.round()).abs() < 0.01 {
        format!("{}", size.round() as i32)
    } else {
        format!("{size:.1}")
    }
}
