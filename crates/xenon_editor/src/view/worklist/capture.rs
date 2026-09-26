//! One title-and-bullets form shared by inline capture and editing.

use super::{EditorView, ItemForm};
use crate::worklist_file::{TITLE_LIMIT, WorkItem, append_item, title_length};
use gpui::{AppContext, Context, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use theme::ActiveTheme;
use xenon_design_system::{OutlineEvent, OutlineView, TypeRole, Typography};

impl EditorView {
    pub(in crate::view) fn worklist_input_open(&self) -> bool {
        self.worklist_capture.is_some() || self.worklist_edit.is_some()
    }

    pub(super) fn worklist_new_form(
        &mut self,
        title: &str,
        details: &str,
        task: bool,
        cx: &mut Context<Self>,
    ) -> ItemForm {
        self.worklist_input_sub.clear();
        let outline = cx.new(|cx| OutlineView::new(title, details, cx));
        outline.update(cx, |outline, cx| outline.open_title(cx));
        self.worklist_input_sub
            .push(cx.subscribe(&outline, |this, _, event, cx| match event {
                OutlineEvent::Changed => cx.notify(),
                OutlineEvent::Submit if this.worklist_capture.is_some() => {
                    this.worklist_save_capture(cx)
                }
                OutlineEvent::Submit => this.worklist_save_edit(cx),
                OutlineEvent::Cancel if this.worklist_capture.is_some() => {
                    this.worklist_cancel_capture(cx)
                }
                OutlineEvent::Cancel => this.worklist_cancel_edit(cx),
            }));
        ItemForm { outline, task }
    }

    pub(super) fn worklist_start_capture(&mut self, cx: &mut Context<Self>) {
        self.worklist_edit = None;
        let form = self.worklist_new_form("", "", true, cx);
        self.worklist_capture = Some(form);
        self.worklist_error = None;
        cx.notify();
    }

    pub(super) fn worklist_cancel_capture(&mut self, cx: &mut Context<Self>) {
        self.worklist_capture = None;
        self.worklist_input_sub.clear();
        cx.notify();
    }

    pub(super) fn worklist_save_capture(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.worklist_capture else {
            return;
        };
        let item = match form.item(cx) {
            Ok(item) => item,
            Err(error) => {
                self.worklist_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let source = self.text();
        let updated = match append_item(&source, &item) {
            Ok(updated) => updated,
            Err(error) => {
                self.worklist_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if self.worklist_mutate(0..source.len(), &updated, cx) {
            self.worklist_selection = super::entries(&updated).len().saturating_sub(1);
            self.worklist_capture = None;
            self.worklist_input_sub.clear();
        }
    }

    pub(super) fn render_worklist_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let form = self.worklist_capture.as_ref().unwrap();
        let can_save = form.item(cx).is_ok();
        div()
            .id("worklist-inline-capture")
            .mb_3()
            .p_3()
            .w_full()
            .max_w(px(850.))
            .rounded_md()
            .border_1()
            .border_color(colors.border_focused)
            .bg(colors.element_background)
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .type_role(TypeRole::Body, cx)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Add a task or note"),
            )
            .child(self.render_worklist_kind(form.task, false, cx))
            .child(form.render_fields(&colors, cx))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(xenon_design_system::action_button(
                        "worklist-inline-cancel",
                        xenon_design_system::ActionButton::secondary("Cancel"),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_cancel_capture(cx)),
                    ))
                    .child(xenon_design_system::action_button(
                        "worklist-inline-save",
                        xenon_design_system::ActionButton::primary("Save").disabled(!can_save),
                        cx,
                        cx.listener(|this, _, _, cx| this.worklist_save_capture(cx)),
                    )),
            )
    }
}

impl ItemForm {
    pub(super) fn item(&self, cx: &Context<EditorView>) -> anyhow::Result<WorkItem> {
        WorkItem::new(
            &self.outline.read(cx).title(cx),
            &self.outline.read(cx).details(cx),
            self.task,
        )
    }

    pub(super) fn render_fields(
        &self,
        colors: &theme::ThemeColors,
        cx: &Context<EditorView>,
    ) -> impl IntoElement + use<> {
        let count = title_length(&self.outline.read(cx).title(cx));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(format!("Title · {count}/{TITLE_LIMIT}")),
            )
            .child(self.outline.clone())
    }
}

impl EditorView {
    pub(super) fn render_worklist_kind(
        &self,
        task: bool,
        editing: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex()
            .gap_2()
            .child(xenon_design_system::action_button(
                "worklist-kind-task",
                if task {
                    xenon_design_system::ActionButton::primary("Task")
                } else {
                    xenon_design_system::ActionButton::secondary("Task")
                },
                cx,
                cx.listener(move |this, _, _, cx| this.worklist_set_kind(true, editing, cx)),
            ))
            .child(xenon_design_system::action_button(
                "worklist-kind-note",
                if task {
                    xenon_design_system::ActionButton::secondary("Note")
                } else {
                    xenon_design_system::ActionButton::primary("Note")
                },
                cx,
                cx.listener(move |this, _, _, cx| this.worklist_set_kind(false, editing, cx)),
            ))
    }

    fn worklist_set_kind(&mut self, task: bool, editing: bool, cx: &mut Context<Self>) {
        if editing {
            if let Some(edit) = &mut self.worklist_edit {
                edit.form.task = task;
            }
        } else if let Some(form) = &mut self.worklist_capture {
            form.task = task;
        }
        cx.notify();
    }
}
