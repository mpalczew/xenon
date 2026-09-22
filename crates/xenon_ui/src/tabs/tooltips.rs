use gpui::{Context, Hsla, IntoElement, ParentElement, Render, SharedString, Styled, div, px};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::icons::icon;

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
    pub(super) background: Hsla,
    pub(super) foreground: Hsla,
    pub(super) accent: Hsla,
    pub(super) focused: bool,
}

impl Render for DragGhost {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(34.))
            .flex_none()
            .min_w_0()
            .border_r_1()
            .border_color(colors.border)
            .bg(self.background)
            .text_color(self.foreground)
            .opacity(0.4)
            .child(
                div()
                    .text_sm()
                    .font_weight(if self.focused {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .max_w(px(160.))
                    .min_w_0()
                    .truncate()
                    .child(self.label.clone()),
            )
            .child(icon(Icon::X, px(12.)))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(2.))
                    .bg(self.accent),
            )
    }
}
