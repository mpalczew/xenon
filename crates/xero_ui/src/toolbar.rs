//! The top toolbar: panel toggles and the active stream's name. Buttons
//! dispatch the same actions as the keyboard shortcuts.

use gpui::{
    Action, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use theme::ActiveTheme;

use crate::app::XeroApp;
use crate::{
    CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, ToggleBrowser, ToggleSidebar,
};

impl XeroApp {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let title = self
            .active_stream()
            .map(|id| self.stream_name(id).to_string());
        let size = xero_settings::font_size(cx) as i32;
        let mut buttons = div().flex().items_center().gap_1();
        buttons = buttons.child(button("tb-sidebar", "☰", Box::new(ToggleSidebar), cx));
        buttons = buttons.child(button("tb-files", "▤", Box::new(ToggleBrowser), cx));
        buttons = buttons.child(button("tb-find", "⌕", Box::new(FilePalette), cx));
        buttons = buttons.child(button("tb-font-dec", "A-", Box::new(DecreaseFontSize), cx));
        buttons = buttons.child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(format!("{size}")),
        );
        buttons = buttons.child(button("tb-font-inc", "A+", Box::new(IncreaseFontSize), cx));
        if self.has_editor() {
            buttons = buttons.child(button(
                "tb-close-editor",
                "✕ editor",
                Box::new(CloseEditor),
                cx,
            ));
        }

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
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .children(title.map(|name| div().child(name))),
            )
    }
}

/// A small toolbar button that dispatches `action` on click.
fn button(
    id: &'static str,
    label: &'static str,
    boxed: Box<dyn Action>,
    cx: &mut Context<XeroApp>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div()
        .id(id)
        .flex()
        .items_center()
        .px_2()
        .py_1()
        .rounded_sm()
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .child(label)
        .on_click(move |_, window: &mut Window, cx| {
            window.dispatch_action(boxed.boxed_clone(), cx);
        })
}
