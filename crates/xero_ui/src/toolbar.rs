//! The top toolbar: panel toggles.

use gpui::{
    Action, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;

use crate::app::XeroApp;
use crate::{ToggleEditor, ToggleSidebar, ToggleTerminal};

enum PanelIcon {
    Left,
    Center,
    Right,
}

impl XeroApp {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let mut buttons = div().flex().items_center().gap_1();
        buttons = buttons.child(button(
            Button {
                id: "tb-sidebar",
                icon: PanelIcon::Left,
                label: "Workspace Sidebar",
                active: self.sidebar_visible(),
                action: Box::new(ToggleSidebar),
            },
            cx,
        ));
        buttons = buttons.child(button(
            Button {
                id: "tb-terminal",
                icon: PanelIcon::Center,
                label: "Terminal Panel",
                active: self.terminal_visible(),
                action: Box::new(ToggleTerminal),
            },
            cx,
        ));
        buttons = buttons.child(button(
            Button {
                id: "tb-editor",
                icon: PanelIcon::Right,
                label: "Editor Panel",
                active: self.editor_visible(),
                action: Box::new(ToggleEditor),
            },
            cx,
        ));

        div()
            .flex()
            .items_center()
            .gap_3()
            .h(px(36.))
            .px_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(buttons)
    }
}

struct Button {
    id: &'static str,
    icon: PanelIcon,
    label: &'static str,
    active: bool,
    action: Box<dyn Action>,
}

/// A small toolbar button that dispatches `action` on click.
fn button(button: Button, cx: &mut Context<XeroApp>) -> impl IntoElement + use<> {
    let Button {
        id,
        icon,
        label,
        active,
        action: boxed,
    } = button;
    let colors = cx.theme().colors().clone();
    let background = if active {
        colors.element_selected
    } else {
        colors.panel_background
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.))
        .h(px(26.))
        .rounded_sm()
        .bg(background)
        .cursor_pointer()
        .hover(move |s| s.bg(colors.element_hover))
        .child(panel_icon(icon, active, cx))
        .tooltip({
            let text = gpui::SharedString::from(label);
            move |_window: &mut Window, cx: &mut gpui::App| {
                cx.new(|_| ToolbarTooltip { text: text.clone() }).into()
            }
        })
        .on_click(move |_, window: &mut Window, cx| {
            window.dispatch_action(boxed.boxed_clone(), cx);
        })
}

fn panel_icon(
    icon: PanelIcon,
    active: bool,
    cx: &mut Context<XeroApp>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let fill = if active {
        colors.text
    } else {
        colors.text_muted
    };
    let empty = colors.panel_background;

    let column = |highlighted| {
        div()
            .w(px(4.))
            .h(px(10.))
            .rounded_sm()
            .bg(if highlighted { fill } else { empty })
    };

    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(2.))
        .w(px(18.))
        .h(px(14.))
        .rounded_sm()
        .border_1()
        .border_color(colors.text_muted)
        .bg(colors.panel_background)
        .child(column(matches!(icon, PanelIcon::Left)))
        .child(column(matches!(icon, PanelIcon::Center)))
        .child(column(matches!(icon, PanelIcon::Right)))
}

struct ToolbarTooltip {
    text: gpui::SharedString,
}

impl gpui::Render for ToolbarTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}
