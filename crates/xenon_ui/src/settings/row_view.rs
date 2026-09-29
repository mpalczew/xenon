//! Paint one setting row: name and detail on the left, its control on the right.

use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{ActionButton, SwitchState, TextInputView, TypeRole, Typography};

use super::SettingsView;
use super::controls::{Choice, choice, swatch_strip};
use super::row::{ActionKind, Control, Field, SettingRow, Tone};
use crate::dropdown::{DropdownProps, dropdown_control, size_stepper};

/// Dropdown and inline-edit state the controls need to paint.
pub(super) struct PaintState<'a> {
    pub open: Option<crate::dropdown::DropdownId>,
    pub filter: &'a str,
    pub filter_input: &'a Entity<TextInputView>,
    pub highlight: usize,
    pub viewport_height: gpui::Pixels,
    pub editing: Option<(Field, Entity<TextInputView>)>,
}

/// Where a row sits: its keyboard index and its place in the card.
#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub index: usize,
    pub focused: bool,
    pub first: bool,
    pub last: bool,
}

pub(super) fn setting_row(
    row: SettingRow,
    at: Placement,
    state: &PaintState<'_>,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let colors = cx.theme().colors().clone();
    let index = at.index;
    let mut shell = div()
        .id(row.id.clone())
        .relative()
        .px_3()
        .py(px(9.))
        .min_h(px(44.))
        .bg(if at.focused {
            colors.element_hover
        } else {
            colors.elevated_surface_background
        })
        .border_x_1()
        .border_b_1()
        .border_color(colors.border)
        .on_click(cx.listener(move |this, _, window, cx| this.focus_row(index, window, cx)));
    if at.first {
        shell = shell.border_t_1().rounded_t_md();
    }
    if at.last {
        shell = shell.rounded_b_md();
    }
    if at.focused {
        shell = shell.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.))
                .bg(colors.text_accent),
        );
    }
    let label = label_block(&row, cx);
    let body = if let Control::Swatches(swatches) = &row.control {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(label)
            .child(swatch_strip(swatches, index, cx))
    } else {
        let control = control(&row, index, state, cx);
        div()
            .flex()
            .items_center()
            .gap_4()
            .child(label)
            .child(control)
    };
    shell.child(body).into_any_element()
}

fn label_block(row: &SettingRow, cx: &mut Context<SettingsView>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let detail = row.detail.clone().map(|detail| {
        let text = div().mt(px(2.));
        match row.tone {
            Tone::Mono => text
                .type_role(TypeRole::Code, cx)
                .text_size(px(12.))
                .text_color(colors.text_muted)
                .children(detail.lines().map(|line| div().child(line.to_string())))
                .into_any_element(),
            tone => text
                .type_role(TypeRole::ControlLabel, cx)
                .font_weight(FontWeight::NORMAL)
                .text_color(if tone == Tone::Good {
                    colors.version_control_added
                } else {
                    colors.text_muted
                })
                .child(detail)
                .into_any_element(),
        }
    });
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().type_role(TypeRole::Body, cx).child(row.label.clone()))
        .children(detail)
}

fn control(
    row: &SettingRow,
    index: usize,
    state: &PaintState<'_>,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    match &row.control {
        Control::None | Control::Swatches(_) => div().into_any_element(),
        Control::Switch { on, toggle } => {
            let toggle = toggle.clone();
            xenon_design_system::switch(
                SharedString::from(format!("{}-switch", row.id)),
                SwitchState {
                    on: *on,
                    disabled: false,
                },
                row.label.clone(),
                cx,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    toggle(window, cx);
                    this.focus_row(index, window, cx);
                }),
            )
            .into_any_element()
        }
        Control::Choice {
            options,
            selected,
            pick,
        } => choice(
            Choice {
                id: &row.id,
                options,
                selected: *selected,
                pick,
            },
            index,
            cx,
        )
        .into_any_element(),
        Control::Font { family, size } => font_controls(*family, *size, state, cx),
        Control::Field {
            field,
            value,
            shown,
        } => field_control(*field, value, shown, state, cx),
        Control::Action(action) => {
            let run = action.run.clone();
            xenon_design_system::action_button(
                SharedString::from(format!("{}-action", row.id)),
                button(action.kind, action.label.clone()),
                cx,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    run(window, cx);
                    this.focus_row(index, window, cx);
                }),
            )
            .into_any_element()
        }
        Control::Value(value) => value_box(value.clone(), cx).into_any_element(),
    }
}

fn button(kind: ActionKind, label: SharedString) -> ActionButton<SharedString> {
    match kind {
        ActionKind::Primary => ActionButton::primary(label),
        ActionKind::Secondary => ActionButton::secondary(label),
        ActionKind::Quiet => ActionButton::quiet(label),
        ActionKind::Destructive => ActionButton::destructive(label),
    }
}

fn font_controls(
    family: crate::dropdown::DropdownId,
    size: crate::dropdown::SizeTarget,
    state: &PaintState<'_>,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let settings = xenon_settings::snapshot(cx);
    let (selected, points, options) = match family {
        crate::dropdown::DropdownId::Ui => (
            settings.ui_font_family,
            settings.ui_font_size,
            crate::dropdown::ui_font_families(cx),
        ),
        crate::dropdown::DropdownId::Editor => (
            settings.editor_font_family,
            settings.editor_font_size,
            crate::dropdown::mono_font_families(cx),
        ),
        crate::dropdown::DropdownId::Terminal => (
            settings.terminal_font_family,
            settings.terminal_font_size,
            crate::dropdown::mono_font_families(cx),
        ),
    };
    let dropdown = dropdown_control(
        DropdownProps {
            id: family,
            selected: &selected,
            options: &options,
            filterable: true,
            open: state.open == Some(family),
            filter: state.filter,
            filter_input: state.filter_input,
            highlight: state.highlight,
            viewport_height: state.viewport_height,
        },
        cx,
    );
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(dropdown)
        .child(size_stepper(size, points, cx))
        .into_any_element()
}

fn field_control(
    field: Field,
    value: &str,
    shown: &SharedString,
    state: &PaintState<'_>,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let colors = cx.theme().colors().clone();
    let frame = div()
        .id(SharedString::from(format!("field-{field:?}")))
        .w(px(300.))
        .h(px(28.))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded_sm()
        .border_1()
        .bg(colors.editor_background)
        .type_role(TypeRole::Code, cx)
        .text_size(px(12.));
    if let Some((_, input)) = state.editing.as_ref().filter(|(f, _)| *f == field) {
        return frame
            .border_color(colors.border_focused)
            .child(div().flex_1().min_w_0().child(input.clone()))
            .into_any_element();
    }
    let current = value.to_string();
    frame
        .border_color(colors.border)
        .cursor_pointer()
        .hover(|s| s.border_color(colors.border_focused))
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            this.begin_field_edit(field, current.clone(), window, cx);
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if value.is_empty() {
                    colors.text_muted
                } else {
                    colors.text
                })
                .child(shown.clone()),
        )
        .child(
            div()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child("Edit"),
        )
        .into_any_element()
}

fn value_box(value: SharedString, cx: &mut Context<SettingsView>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div()
        .min_w(px(70.))
        .h(px(28.))
        .px_2()
        .flex()
        .items_center()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.editor_background)
        .type_role(TypeRole::Code, cx)
        .text_size(px(12.))
        .text_color(colors.text_muted)
        .child(value)
}
