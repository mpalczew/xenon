use super::*;

impl EditorView {
    pub(super) fn render_worklist_edit(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let edit = self.worklist_edit.as_ref().unwrap();
        let label = if edit.entry.checked.is_some() {
            "Edit task and notes"
        } else {
            "Edit note"
        };
        let at = self.worklist_selection;
        let count = entries(&self.text()).len();
        div()
            .id("worklist-item-edit")
            .flex_1()
            .min_h_0()
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .px_4()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(div().id("worklist-item-field").child(edit.input.clone()))
            .child(self.render_edit_actions(colors, cx))
            .child(div().text_xs().text_color(colors.text_muted).child(format!(
                "Item {} of {} · ⌘Enter save · Esc cancel",
                at + 1,
                count
            )))
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
                    .child(
                        div()
                            .id("worklist-move-up")
                            .px_2()
                            .py_1()
                            .border_1()
                            .border_color(colors.border)
                            .child("↑ Move up")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_move(-1, cx))),
                    )
                    .child(
                        div()
                            .id("worklist-move-down")
                            .px_2()
                            .py_1()
                            .border_1()
                            .border_color(colors.border)
                            .child("↓ Move down")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_move(1, cx))),
                    )
                    .child(
                        div()
                            .id("worklist-delete")
                            .px_2()
                            .py_1()
                            .text_color(colors.version_control_deleted)
                            .child("Delete")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_delete(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("worklist-cancel")
                            .px_3()
                            .py_1()
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_cancel_edit(cx))),
                    )
                    .child(
                        div()
                            .id("worklist-save-item")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .bg(colors.element_active)
                            .child("Save changes")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_save_edit(cx))),
                    ),
            )
    }
}
