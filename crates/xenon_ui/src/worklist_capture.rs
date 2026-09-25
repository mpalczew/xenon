//! Compact keyboard-first capture surface for a workspace worklist.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TextInputConfig, TextInputEvent, TextInputView};

#[cfg(feature = "visual-tests")]
mod visual;

pub enum CaptureEvent {
    Submit { text: String },
    Dismissed,
}

pub struct WorklistCaptureView {
    input: Entity<TextInputView>,
    draft: String,
    focus: FocusHandle,
    _input_subscription: Subscription,
    error: Option<String>,
    submitting: bool,
    workspace_name: String,
}

impl EventEmitter<CaptureEvent> for WorklistCaptureView {}

impl WorklistCaptureView {
    pub fn new(workspace_name: String, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::multiline("What needs doing?", px(72.)).submit_on_plain_enter(),
                cx,
            )
        });
        let focus = input.read(cx).focus_handle();
        let input_subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            TextInputEvent::Changed(text) => {
                this.draft = text.clone();
                this.error = None;
                cx.notify();
            }
            TextInputEvent::Submit(text) => this.submit(text, cx),
            TextInputEvent::Cancel => cx.emit(CaptureEvent::Dismissed),
        });
        Self {
            input,
            draft: String::new(),
            focus,
            _input_subscription: input_subscription,
            error: None,
            submitting: false,
            workspace_name,
        }
    }

    pub fn saved(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text("", cx));
        self.draft.clear();
        self.submitting = false;
        self.error = None;
        cx.notify();
    }

    pub fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.submitting = false;
        cx.notify();
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.open(cx));
        cx.notify();
    }

    fn submit(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if !text.is_empty() && !self.submitting {
            self.submitting = true;
            cx.emit(CaptureEvent::Submit {
                text: text.to_owned(),
            });
        }
    }

    fn save_button(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text = self.draft.clone();
        let active = !text.trim().is_empty();
        div().mt_2().flex().justify_end().child(
            div()
                .id("worklist-capture-save")
                .px_3()
                .py_1()
                .rounded_md()
                .bg(if active {
                    colors.element_active
                } else {
                    colors.element_background
                })
                .text_color(colors.text)
                .text_sm()
                .cursor_pointer()
                .child("Save")
                .on_click(cx.listener(move |this, _, _, cx| this.submit(&text, cx))),
        )
    }
}

impl Focusable for WorklistCaptureView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WorklistCaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let panel = div()
            .absolute()
            .top(px(48.))
            .right(px(12.))
            .w(px(360.))
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .text_color(colors.text)
            .shadow_lg()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Add a Task"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(self.workspace_name.clone()),
            )
            .child(div().mt_2().child(self.input.clone()))
            .child(
                div()
                    .mt_2()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Enter to save  ·  ⇧Enter for a new line  ·  Esc to keep draft"),
            )
            .child(self.save_button(&colors, cx))
            .children(self.error.as_ref().map(|error| {
                div()
                    .mt_2()
                    .text_xs()
                    .text_color(colors.version_control_deleted)
                    .child(error.clone())
            }))
            .id("worklist-capture-panel")
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()));
        div()
            .absolute()
            .inset_0()
            .id("worklist-capture-scrim")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(CaptureEvent::Dismissed)))
            .child(panel)
    }
}
