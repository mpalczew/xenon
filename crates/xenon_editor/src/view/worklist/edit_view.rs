use super::*;
use xenon_design_system::{ActionButton, action_button};

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
                action_button(
                    "worklist-delete",
                    ActionButton::destructive("Delete"),
                    cx,
                    cx.listener(|this, _, _, cx| this.worklist_delete(cx)),
                )
                .self_start(),
            )
    }
}
