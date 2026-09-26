//! Shared action target. The label and button type role live here, so a caller
//! cannot leave the font to the surrounding screen. GPUI synthesizes
//! `ClickEvent::Keyboard` for focused Enter/Space activation, so one handler
//! covers pointer and keyboard input.

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;

use crate::typography::{TypeRole, Typography};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionButtonVariant {
    Primary,
    Secondary,
    Quiet,
    Icon,
}

/// Kind, label, and enabled state for one action.
pub struct ActionButton<L> {
    variant: ActionButtonVariant,
    label: L,
    disabled: bool,
}

impl<L> ActionButton<L> {
    pub fn primary(label: L) -> Self {
        Self::new(ActionButtonVariant::Primary, label)
    }

    pub fn secondary(label: L) -> Self {
        Self::new(ActionButtonVariant::Secondary, label)
    }

    pub fn quiet(label: L) -> Self {
        Self::new(ActionButtonVariant::Quiet, label)
    }

    pub fn icon(label: L) -> Self {
        Self::new(ActionButtonVariant::Icon, label)
    }

    pub fn new(variant: ActionButtonVariant, label: L) -> Self {
        Self {
            variant,
            label,
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

pub fn action_button<L: IntoElement>(
    id: impl Into<ElementId>,
    button: ActionButton<L>,
    cx: &App,
    on_activate: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let ActionButton {
        variant,
        label,
        disabled,
    } = button;
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    let (background, foreground, border) = match variant {
        ActionButtonVariant::Primary => {
            (colors.element_active, colors.text, colors.border_selected)
        }
        ActionButtonVariant::Secondary => (colors.element_background, colors.text, colors.border),
        ActionButtonVariant::Quiet => {
            (gpui::transparent_black(), accent, gpui::transparent_black())
        }
        ActionButtonVariant::Icon => (
            gpui::transparent_black(),
            colors.icon,
            gpui::transparent_black(),
        ),
    };
    let hover = if variant == ActionButtonVariant::Primary {
        background.blend(accent.opacity(0.12))
    } else {
        colors.element_hover
    };
    let pressed = if variant == ActionButtonVariant::Primary {
        background.blend(accent.opacity(0.28))
    } else {
        colors.element_hover.blend(accent.opacity(0.18))
    };
    let button = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(32.))
        .px_3()
        .rounded_sm()
        .bg(background)
        .focusable()
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(colors.border_focused))
        .type_role(TypeRole::Button, cx)
        .text_color(foreground);
    let button = if matches!(
        variant,
        ActionButtonVariant::Primary | ActionButtonVariant::Secondary
    ) {
        button.border_1().border_color(border)
    } else {
        button
    };
    let button = if variant == ActionButtonVariant::Icon {
        button.w(px(32.)).px_0()
    } else {
        button
    };
    if disabled {
        button.opacity(0.45).tab_index(-1).child(label)
    } else {
        button
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed).top(px(2.)))
            .on_click(on_activate)
            .child(label)
    }
}
