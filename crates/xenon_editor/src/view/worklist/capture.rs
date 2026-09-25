//! Inline capture in the worklist tab. The file remains the only data store.

use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TextInputConfig, TextInputEvent, TextInputView};

use super::EditorView;

impl EditorView {
    pub(in crate::view) fn worklist_input(&self) -> Option<&Entity<TextInputView>> {
        self.worklist_capture
            .as_ref()
            .or_else(|| self.worklist_edit.as_ref().map(|edit| &edit.input))
    }

    pub(super) fn worklist_new_input(
        &mut self,
        config: TextInputConfig,
        initial: &str,
        cx: &mut Context<Self>,
    ) -> Entity<TextInputView> {
        let input = cx.new(|cx| TextInputView::new(config, cx));
        input.update(cx, |input, cx| {
            input.set_text(initial, cx);
            input.open(cx);
        });
        self.worklist_input_sub = Some(cx.subscribe(&input, |this, _, event, cx| match event {
            TextInputEvent::Changed(_) => cx.notify(),
            TextInputEvent::Submit(text) if this.worklist_capture.is_some() => {
                this.worklist_save_capture_text(text, cx)
            }
            TextInputEvent::Submit(text) if this.worklist_edit.is_some() => {
                this.worklist_save_edit_text(text, cx)
            }
            TextInputEvent::Cancel if this.worklist_capture.is_some() => {
                this.worklist_cancel_capture(cx)
            }
            TextInputEvent::Cancel if this.worklist_edit.is_some() => this.worklist_cancel_edit(cx),
            _ => {}
        }));
        input
    }

    pub(super) fn worklist_start_capture(&mut self, cx: &mut Context<Self>) {
        self.worklist_edit = None;
        self.worklist_input_sub = None;
        let input = self.worklist_new_input(
            TextInputConfig::multiline("What needs doing?", px(92.)).submit_on_plain_enter(),
            "",
            cx,
        );
        self.worklist_capture = Some(input);
        self.worklist_error = None;
        cx.notify();
    }

    pub(super) fn worklist_cancel_capture(&mut self, cx: &mut Context<Self>) {
        self.worklist_capture = None;
        self.worklist_input_sub = None;
        cx.notify();
    }

    pub(super) fn worklist_save_capture(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.worklist_capture.as_ref() else {
            return;
        };
        let text = input.read(cx).text().to_owned();
        self.worklist_save_capture_text(&text, cx);
    }

    fn worklist_save_capture_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let source = self.text();
        let updated = match crate::worklist_file::prepare_append(&source, text, true) {
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
            self.worklist_input_sub = None;
        }
    }

    pub(super) fn render_worklist_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let input = self.worklist_capture.as_ref().unwrap();
        let can_save = !input.read(cx).text().trim().is_empty();
        div()
            .id("worklist-inline-capture")
            .mb_3()
            .p_3()
            .w_full()
            .max_w(px(760.))
            .rounded_md()
            .border_1()
            .border_color(colors.border_focused)
            .bg(colors.element_background)
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Add a task"),
            )
            .child(input.clone())
            .child(self.render_capture_actions(can_save, &colors, cx))
    }

    fn render_capture_actions(
        &self,
        can_save: bool,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Enter save · ⇧Enter line · Esc cancel"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("worklist-inline-cancel")
                            .px_2()
                            .py_1()
                            .child("Cancel")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.worklist_cancel_capture(cx)),
                            ),
                    )
                    .child(
                        div()
                            .id("worklist-inline-save")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .bg(if can_save {
                                colors.element_active
                            } else {
                                colors.element_background
                            })
                            .child("Save")
                            .on_click(cx.listener(|this, _, _, cx| this.worklist_save_capture(cx))),
                    ),
            )
    }
}
