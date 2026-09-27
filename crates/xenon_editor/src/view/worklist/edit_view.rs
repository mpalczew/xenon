use super::*;
use gpui::{App, AppContext, IntoElement, ParentElement, Render, SharedString, Styled, Window};
use theme::ActiveTheme;
use xenon_design_system::{ActionButton, TypeRole, Typography, action_button};

impl EditorView {
    pub(super) fn render_worklist_edit(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let edit = self.worklist_edit.as_ref().unwrap();
        div()
            .id("worklist-item-edit")
            .mb_3()
            .w_full()
            .max_w(px(850.))
            .rounded_md()
            .border_1()
            .border_color(colors.border_focused)
            .bg(colors.element_background)
            .px_3()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(edit.form.render_fields())
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        action_button(
                            "worklist-close",
                            ActionButton::secondary("Close"),
                            cx,
                            cx.listener(|this, _, _, cx| this.worklist_close_edit(cx)),
                        )
                        .self_start()
                        .tooltip(action_hint("Close", "Esc")),
                    )
                    .child(
                        action_button(
                            "worklist-delete",
                            ActionButton::destructive("Delete"),
                            cx,
                            cx.listener(|this, _, _, cx| this.worklist_delete(cx)),
                        )
                        .self_start()
                        .tooltip(action_hint("Delete", "⌘⌫")),
                    ),
            )
    }
}

fn action_hint(
    label: &'static str,
    keys: &'static str,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    move |_window, cx| {
        cx.new(|_| ActionHint {
            label: label.into(),
            keys: keys.into(),
        })
        .into()
    }
}

struct ActionHint {
    label: SharedString,
    keys: SharedString,
}

impl Render for ActionHint {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .flex()
            .items_center()
            .gap_2()
            .type_role(TypeRole::Body, cx)
            .child(self.label.clone())
            .child(
                div()
                    .px_1()
                    .rounded_xs()
                    .bg(colors.element_background)
                    .type_role(TypeRole::ControlLabel, cx)
                    .child(self.keys.clone()),
            )
    }
}
