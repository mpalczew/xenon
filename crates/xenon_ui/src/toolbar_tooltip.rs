//! Hover and focus labels for toolbar icon buttons.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

pub(crate) struct ToolbarTooltip {
    pub label: gpui::SharedString,
    pub keys: Option<gpui::SharedString>,
}

impl Render for ToolbarTooltip {
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
            .type_role(TypeRole::Body, cx)
            .flex()
            .items_center()
            .gap_2()
            .child(self.label.clone())
            .children(self.keys.as_ref().map(|keys| {
                div()
                    .px_1()
                    .rounded_xs()
                    .bg(colors.element_background)
                    .text_color(colors.text_muted)
                    .child(keys.clone())
            }))
    }
}
