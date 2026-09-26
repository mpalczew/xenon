use super::*;
use xenon_design_system::{ActionButton, TypeRole, Typography, action_button};

impl EditorView {
    pub(super) fn render_worklist_edit(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let edit = self.worklist_edit.as_ref().unwrap();
        let label = "Edit work item";
        let at = self.worklist_selection;
        let count = entries(&self.text()).len();
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
            .child(
                div()
                    .type_role(TypeRole::Body, cx)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(self.render_worklist_kind(edit.form.task, true, cx))
            .child(edit.form.render_fields(colors, cx))
            .child(self.render_edit_actions(colors, cx))
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(format!(
                        "Item {} of {} · ⌘Enter save · Esc cancel",
                        at + 1,
                        count
                    )),
            )
    }

    fn render_edit_actions(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(action_button(
                        "worklist-move-up",
                        ActionButton::secondary("↑ Move up"),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_move(-1, cx)),
                    ))
                    .child(action_button(
                        "worklist-move-down",
                        ActionButton::secondary("↓ Move down"),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_move(1, cx)),
                    ))
                    .child(
                        action_button(
                            "worklist-delete",
                            ActionButton::quiet("Delete"),
                            cx,
                            cx.listener(|this, _, _, cx| this.worklist_delete(cx)),
                        )
                        .text_color(colors.version_control_deleted),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(action_button(
                        "worklist-cancel",
                        ActionButton::secondary("Cancel"),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_cancel_edit(cx)),
                    ))
                    .child(action_button(
                        "worklist-save-item",
                        ActionButton::primary("Save changes"),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_save_edit(cx)),
                    )),
            )
    }
}
