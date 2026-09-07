//! Empty content state: explain the next useful action without a tour.

use gpui::{
    AnyElement, Context, Image, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, img, px,
};
use std::sync::Arc;

use super::XenonApp;

fn xenon_icon() -> Arc<Image> {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../../../macos/xenon-icon-source.png").to_vec(),
    ))
}

impl XenonApp {
    pub(super) fn render_empty_state(
        &self,
        colors: theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let has_workspace = self.active.is_some();
        let title = if has_workspace {
            "Your workspace is ready"
        } else {
            "Open a workspace to get started"
        };
        let subtitle = if has_workspace {
            "Start a terminal or open a file. Your surfaces will appear here."
        } else {
            "Xenon keeps your agent work organized across workspaces."
        };
        let primary = if has_workspace {
            let hover = colors.element_hover;
            div()
                .id("empty-new-terminal")
                .px_3()
                .py_2()
                .rounded_sm()
                .bg(colors.element_selected)
                .text_color(colors.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child("New Terminal  ⌘N")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.new_terminal(window, cx);
                }))
                .into_any_element()
        } else {
            let hover = colors.element_hover;
            div()
                .id("empty-open-workspace")
                .px_3()
                .py_2()
                .rounded_sm()
                .bg(colors.element_selected)
                .text_color(colors.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child("Open Workspace  ⌘⇧O")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.add_workspace(window, cx);
                }))
                .into_any_element()
        };
        let secondary = has_workspace.then(|| {
            let hover = colors.element_hover;
            let text = colors.text;
            div()
                .id("empty-open-file")
                .px_3()
                .py_2()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(hover).text_color(text))
                .child("Open File  ⌘P")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_palette(window, cx);
                }))
                .into_any_element()
        });
        empty_state_content(colors, title, subtitle, primary, secondary)
    }
}

fn empty_state_content(
    colors: theme::ThemeColors,
    title: &'static str,
    subtitle: &'static str,
    primary: AnyElement,
    secondary: Option<AnyElement>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .size_full()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .max_w(px(420.))
                .text_center()
                .child(img(xenon_icon()).size(px(72.)))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(colors.text)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.text_muted)
                        .child(subtitle),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .pt_2()
                        .child(primary)
                        .children(secondary),
                ),
        )
}
