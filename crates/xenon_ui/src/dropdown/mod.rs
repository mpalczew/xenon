//! Filterable / plain dropdown + size stepper for the Settings window.
//! Open lists are deferred window-anchored popovers beside the trigger. A
//! filterable trigger turns into its own filter field while open, so typing
//! happens where the control sits. The list's side is chosen once, on open,
//! from the trigger's last painted bounds, so filtering never moves it.

use xenon_design_system::{TypeRole, Typography};
mod fonts;

use fonts::{family_option_label, format_size};
pub(crate) use fonts::{
    mono_family_label, mono_font_families, ui_font_families, warm_mono_font_families,
    warm_ui_font_families,
};

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Anchor, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, anchored, canvas, deferred, div, point,
    prelude::FluentBuilder, px, relative,
};
use theme::ActiveTheme;
use xenon_design_system::TextInputView;

use crate::settings::SettingsView;

/// Which font family list is open (at most one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropdownId {
    Ui,
    Editor,
    Terminal,
}

impl DropdownId {
    fn slot(self) -> usize {
        match self {
            Self::Ui => 0,
            Self::Editor => 1,
            Self::Terminal => 2,
        }
    }
}

/// Each trigger's last painted window bounds, read when its list opens.
#[derive(Clone, Default)]
pub(crate) struct TriggerBounds(Rc<Cell<[Option<Bounds<Pixels>>; 3]>>);

impl TriggerBounds {
    fn record(&self, id: DropdownId, bounds: Bounds<Pixels>) {
        let mut all = self.0.get();
        all[id.slot()] = Some(bounds);
        self.0.set(all);
    }

    /// Open upward only when the list does not fit below and more room is above.
    /// None until the trigger has painted.
    pub(crate) fn opens_up(&self, id: DropdownId, viewport_height: Pixels) -> Option<bool> {
        let bounds = self.0.get()[id.slot()]?;
        let below = viewport_height - bounds.bottom();
        let above = bounds.top();
        Some(below < popup_max_height(viewport_height) + px(12.) && above > below)
    }
}

/// The chosen family as its trigger shows it; the open filter's placeholder.
pub(crate) fn current_label(id: DropdownId, cx: &gpui::App) -> String {
    let settings = xenon_settings::snapshot(cx);
    let family = match id {
        DropdownId::Ui => settings.ui_font_family,
        DropdownId::Editor => settings.editor_font_family,
        DropdownId::Terminal => settings.terminal_font_family,
    };
    family_option_label(id, &family)
}

/// Which surface a size stepper adjusts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SizeTarget {
    Ui,
    Editor,
    Terminal,
}

/// Shared width for the trigger and the floating list.
pub(crate) const PANEL_WIDTH: f32 = 220.;
const LIST_MAX: f32 = 280.;
const LIST_MIN: f32 = 96.;

pub(crate) struct DropdownProps<'a> {
    pub id: DropdownId,
    pub selected: &'a str,
    pub options: &'a [SharedString],
    pub filterable: bool,
    pub open: bool,
    pub filter: &'a str,
    pub filter_input: &'a gpui::Entity<TextInputView>,
    pub highlight: usize,
    pub viewport_height: Pixels,
    pub bounds: TriggerBounds,
    pub opens_up: bool,
}

/// Trigger; the open list is a deferred popover on the side chosen at open.
pub(crate) fn dropdown_control(
    props: DropdownProps<'_>,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let filtered = filter_options(props.options, props.filterable, props.filter);
    let max_h = popup_max_height(props.viewport_height);
    let (id, bounds) = (props.id, props.bounds.clone());

    div()
        .id(SharedString::from(format!("dd-panel-{:?}", props.id)))
        .relative()
        .w(px(PANEL_WIDTH))
        .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .child(
            canvas(move |b, _, _| bounds.record(id, b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(if props.open && props.filterable {
            filter_trigger(props.filter_input.clone(), cx).into_any_element()
        } else {
            trigger(props.id, props.selected, props.open, cx).into_any_element()
        })
        // Zero-height strip on the chosen edge of the trigger: the popover origin,
        // so the list opens this frame (no bounds-tracker wait).
        .when(props.open, |panel| {
            let list = option_list(
                ListProps {
                    id: props.id,
                    options: &filtered,
                    selected: props.selected,
                    highlight: props.highlight,
                    max_h,
                },
                cx,
            );
            let (edge, anchor, gap) = if props.opens_up {
                (relative(0.), Anchor::BottomLeft, px(-4.))
            } else {
                (relative(1.), Anchor::TopLeft, px(4.))
            };
            panel.child(
                div().absolute().top(edge).left_0().w_full().h(px(0.)).child(
                    deferred(
                        anchored()
                            .anchor(anchor)
                            .offset(point(px(0.), gap))
                            // Never flip on its own: flipping would cover the field.
                            .snap_to_window()
                            .child(div().occlude().w(px(PANEL_WIDTH)).child(list)),
                    )
                    .with_priority(100),
                ),
            )
        })
}

fn popup_max_height(viewport_height: Pixels) -> Pixels {
    // Half the window so either drop-down or flip-up can fit.
    let half = viewport_height / 2. - px(8.);
    half.max(px(LIST_MIN)).min(px(LIST_MAX))
}

/// `[−] 14 [+]` size control (not a per-point dropdown).
pub(crate) fn size_stepper(
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
                .type_role(TypeRole::ControlLabel, cx)
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
    xenon_design_system::action_button(
        SharedString::from(format!("size-{target:?}-{delta}")),
        xenon_design_system::ActionButton::secondary(label),
        cx,
        cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.nudge_font_size(target, delta, cx);
        }),
    )
    .w(px(28.))
}

fn trigger(
    id: DropdownId,
    selected: &str,
    open: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let label = family_option_label(id, selected);
    let chevron = if open { "▴" } else { "▾" };
    xenon_design_system::action_button(
        SharedString::from(format!("dd-trigger-{id:?}")),
        xenon_design_system::ActionButton::secondary(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(label),
        ),
        cx,
        cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            this.toggle_dropdown(id, window, cx);
        }),
    )
    .flex()
    .items_center()
    .justify_between()
    .gap_2()
    .w_full()
    .child(div().text_color(colors.text_muted).child(chevron))
}

struct ListProps<'a> {
    id: DropdownId,
    options: &'a [SharedString],
    selected: &'a str,
    highlight: usize,
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

    if props.options.is_empty() {
        list = list.child(
            div()
                .px_2()
                .py_2()
                .type_role(TypeRole::ControlLabel, cx)
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

/// The open, filterable trigger: same box as the button, holding the filter field.
fn filter_trigger(
    input: gpui::Entity<TextInputView>,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .min_h(px(32.))
        .px_3()
        .rounded_sm()
        .border_1()
        .border_color(colors.border_selected)
        .bg(colors.element_background)
        .type_role(TypeRole::Button, cx)
        .child(div().flex_1().min_w_0().child(input))
        .child(div().text_color(colors.text_muted).child("▴"))
}

fn option_row(
    id: DropdownId,
    option: SharedString,
    selected: bool,
    highlighted: bool,
    cx: &mut gpui::Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    // Same multi-channel selection language as elevated palettes / sidebar.
    let paint = crate::chrome::list_selection(&colors, highlighted);
    let background = if highlighted {
        paint.background
    } else if selected {
        colors.element_hover
    } else {
        colors.elevated_surface_background
    };
    let foreground = if highlighted {
        paint.foreground
    } else {
        colors.text
    };
    let pick = option.clone();
    let label = family_option_label(id, option.as_ref());
    div()
        .id(SharedString::from(format!("dd-opt-{id:?}-{option}")))
        .px_2()
        .py_1()
        .type_role(TypeRole::ControlLabel, cx)
        .bg(background)
        .text_color(foreground)
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
                || xenon_settings::display_ui_family(name.as_ref())
                    .to_lowercase()
                    .contains(&needle)
        })
        .cloned()
        .collect()
}
