//! Filterable / plain dropdown + size stepper for the Settings window.
//! Open lists are deferred window-anchored popovers under the trigger.

use std::sync::Mutex;

use gpui::{
    Anchor, App, InteractiveElement, IntoElement, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, anchored, deferred, div, point, prelude::FluentBuilder, px,
    relative,
};
use theme::{ActiveTheme, FontFamilyCache};

use crate::settings::SettingsView;

/// Which settings dropdown is open (at most one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropdownId {
    Mode,
    LightTheme,
    DarkTheme,
    EditorFamily,
    TerminalFamily,
    TerminalAutoClose,
}

/// Which surface a size stepper adjusts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SizeTarget {
    Editor,
    Terminal,
}

/// Shared width for the trigger and the floating list.
pub(crate) const PANEL_WIDTH: f32 = 220.;
const LIST_MAX: f32 = 280.;
const LIST_MIN: f32 = 96.;

pub(crate) struct DropdownProps<'a> {
    pub id: DropdownId,
    pub title: &'static str,
    pub selected: &'a str,
    pub options: &'a [SharedString],
    pub filterable: bool,
    pub open: bool,
    pub filter: &'a str,
    pub highlight: usize,
    pub caret_on: bool,
    pub viewport_height: Pixels,
}

/// Compact label + trigger; open list is a deferred popover that flips to stay on screen.
pub(crate) fn dropdown_row(
    props: DropdownProps<'_>,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let filtered = filter_options(props.options, props.filterable, props.filter);
    let max_h = popup_max_height(props.viewport_height);

    let panel = div()
        .id(SharedString::from(format!("dd-panel-{:?}", props.id)))
        .relative()
        .w(px(PANEL_WIDTH))
        .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .child(trigger(props.id, props.selected, props.open, cx))
        // Zero-height strip at the bottom of the trigger: popover origin so the
        // list opens this frame (no bounds-tracker wait).
        .when(props.open, |panel| {
            let list = option_list(
                ListProps {
                    id: props.id,
                    options: &filtered,
                    selected: props.selected,
                    filterable: props.filterable,
                    filter: props.filter,
                    highlight: props.highlight,
                    caret_on: props.caret_on,
                    max_h,
                },
                cx,
            );
            panel.child(
                div()
                    .absolute()
                    .top(relative(1.))
                    .left_0()
                    .w_full()
                    .h(px(0.))
                    .child(
                        deferred(
                            anchored()
                                .anchor(Anchor::TopLeft)
                                // Gap below the trigger; SwitchAnchor flips upward when
                                // the list would overflow the bottom of the window.
                                .offset(point(px(0.), px(4.)))
                                .child(div().occlude().w(px(PANEL_WIDTH)).child(list)),
                        )
                        .with_priority(100),
                    ),
            )
        });

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
        .child(div().text_sm().text_color(colors.text).child(props.title))
        .child(panel)
}

fn popup_max_height(viewport_height: Pixels) -> Pixels {
    // Half the window so either drop-down or flip-up can fit.
    let half = viewport_height / 2. - px(8.);
    half.max(px(LIST_MIN)).min(px(LIST_MAX))
}

/// Label + `[−] 14 [+]` size control (not a per-point dropdown).
pub(crate) fn size_row(
    title: &'static str,
    target: SizeTarget,
    size: f32,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .id(SharedString::from(format!("size-row-{target:?}")))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
        .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .child(div().text_sm().text_color(colors.text).child(title))
        .child(size_stepper(target, size, cx))
}

fn size_stepper(
    target: SizeTarget,
    size: f32,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let label = format_size(size);
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(step_btn(target, -1.0, "−", cx))
        .child(
            div()
                .min_w(px(36.))
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .bg(colors.elevated_surface_background)
                .text_xs()
                .flex()
                .items_center()
                .justify_center()
                .child(label),
        )
        .child(step_btn(target, 1.0, "+", cx))
}

fn step_btn(
    target: SizeTarget,
    delta: f32,
    label: &'static str,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .id(SharedString::from(format!("size-{target:?}-{delta}")))
        .w(px(28.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .text_xs()
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.nudge_font_size(target, delta, cx);
        }))
}

fn trigger(
    id: DropdownId,
    selected: &str,
    open: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let label = if is_mono_family_dropdown(id) {
        mono_family_label(selected)
    } else {
        selected.to_string()
    };
    let chevron = if open { "▴" } else { "▾" };
    div()
        .id(SharedString::from(format!("dd-trigger-{id:?}")))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .w_full()
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
            cx.stop_propagation();
            this.toggle_dropdown(id, window, cx);
        }))
}

struct ListProps<'a> {
    id: DropdownId,
    options: &'a [SharedString],
    selected: &'a str,
    filterable: bool,
    filter: &'a str,
    highlight: usize,
    caret_on: bool,
    max_h: Pixels,
}

fn option_list(props: ListProps<'_>, cx: &mut gpui::Context<SettingsView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let mut list = div()
        .id(SharedString::from(format!("dd-list-{:?}", props.id)))
        .flex()
        .flex_col()
        .w_full()
        .max_h(props.max_h)
        .overflow_y_scroll()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .shadow_sm()
        .on_scroll_wheel(cx.listener(|_, _, _, cx| {
            cx.stop_propagation();
        }))
        .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()));

    if props.filterable {
        list = list.child(filter_banner(props.filter, props.caret_on, cx));
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

fn filter_banner(
    filter: &str,
    caret_on: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let empty = filter.is_empty();
    let mut row = div()
        .flex()
        .items_center()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(colors.border)
        .text_xs();
    row = row.child(div().w(px(1.)).h(px(12.)).mr_0p5().bg(if caret_on {
        colors.text
    } else {
        gpui::transparent_black()
    }));
    if empty {
        row = row.child(div().text_color(colors.text_muted).child("Type to filter…"));
    } else {
        row = row.child(div().text_color(colors.text).child(filter.to_string()));
    }
    row
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
    let label = if is_mono_family_dropdown(id) {
        mono_family_label(option.as_ref())
    } else {
        option.to_string()
    };
    div()
        .id(SharedString::from(format!("dd-opt-{id:?}-{option}")))
        .px_2()
        .py_1()
        .text_xs()
        .bg(background)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
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
        .filter(|name| {
            name.to_lowercase().contains(&needle)
                || mono_family_label(name.as_ref())
                    .to_lowercase()
                    .contains(&needle)
        })
        .cloned()
        .collect()
}

/// Monospace families only. Cached after first measurement (advance checks are costly).
pub(crate) fn mono_font_families(cx: &App) -> Vec<SharedString> {
    static CACHE: Mutex<Option<Vec<SharedString>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached) = guard.as_ref() {
        return cached.clone();
    }
    let list: Vec<SharedString> = FontFamilyCache::global(cx)
        .list_font_families(cx)
        .into_iter()
        .filter(|name| xero_settings::is_monospace_family(name.as_ref(), cx))
        .collect();
    *guard = Some(list.clone());
    list
}

fn is_mono_family_dropdown(id: DropdownId) -> bool {
    matches!(id, DropdownId::EditorFamily | DropdownId::TerminalFamily)
}

fn mono_family_label(family: &str) -> String {
    xero_settings::display_mono_family(family).to_string()
}

pub(crate) fn format_size(size: f32) -> String {
    if (size - size.round()).abs() < 0.01 {
        format!("{}", size.round() as i32)
    } else {
        format!("{size:.1}")
    }
}
