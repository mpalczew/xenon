use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, div};
use theme::ActiveTheme;

pub(super) struct TabTooltip {
    pub(super) text: SharedString,
}

impl Render for TabTooltip {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}

pub(super) struct DragGhost {
    pub(super) label: SharedString,
}

impl Render for DragGhost {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .child(self.label.clone())
    }
}
