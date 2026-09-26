//! Compact title-and-bullets capture for a workspace worklist.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{
    ActionButton, OutlineEvent, OutlineView, TypeRole, Typography, action_button,
};
use xenon_editor::worklist_file::{TITLE_LIMIT, WorkItem, title_length};

#[cfg(feature = "visual-tests")]
mod visual;

pub enum CaptureEvent {
    Submit { item: WorkItem },
    Dismissed,
}

pub struct WorklistCaptureView {
    outline: Entity<OutlineView>,
    task: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    error: Option<String>,
    submitting: bool,
    workspace_name: String,
}

impl EventEmitter<CaptureEvent> for WorklistCaptureView {}

impl WorklistCaptureView {
    pub fn new(workspace_name: String, cx: &mut Context<Self>) -> Self {
        let outline = cx.new(|cx| OutlineView::new("", "", cx));
        let focus = outline.read(cx).focus_handle(cx);
        let subscriptions = vec![cx.subscribe(&outline, |this, _, event, cx| match event {
            OutlineEvent::Changed => {
                this.error = None;
                cx.notify();
            }
            OutlineEvent::Submit => this.submit(cx),
            OutlineEvent::Cancel => cx.emit(CaptureEvent::Dismissed),
        })];
        Self {
            outline,
            task: true,
            focus,
            _subscriptions: subscriptions,
            error: None,
            submitting: false,
            workspace_name,
        }
    }

    pub fn saved(&mut self, cx: &mut Context<Self>) {
        self.outline.update(cx, |outline, cx| outline.clear(cx));
        self.task = true;
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
        self.outline
            .update(cx, |outline, cx| outline.open_title(cx));
        cx.notify();
    }

    fn item(&self, cx: &Context<Self>) -> anyhow::Result<WorkItem> {
        WorkItem::new(
            &self.outline.read(cx).title(cx),
            &self.outline.read(cx).details(cx),
            self.task,
        )
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        match self.item(cx) {
            Ok(item) => {
                self.submitting = true;
                cx.emit(CaptureEvent::Submit { item });
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn kind_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .mt_2()
            .flex()
            .gap_2()
            .child(action_button(
                "capture-task",
                if self.task {
                    ActionButton::primary("Task")
                } else {
                    ActionButton::secondary("Task")
                },
                cx,
                cx.listener(|this, _, _, cx| {
                    this.task = true;
                    cx.notify();
                }),
            ))
            .child(action_button(
                "capture-note",
                if self.task {
                    ActionButton::secondary("Note")
                } else {
                    ActionButton::primary("Note")
                },
                cx,
                cx.listener(|this, _, _, cx| {
                    this.task = false;
                    cx.notify();
                }),
            ))
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
        let count = title_length(&self.outline.read(cx).title(cx));
        let can_save = self.item(cx).is_ok();
        let panel = div()
            .absolute()
            .top(px(48.))
            .right(px(12.))
            .w(px(430.))
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .text_color(colors.text)
            .shadow_lg()
            .child(
                div()
                    .type_role(TypeRole::Body, cx)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Quick capture"),
            )
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(self.workspace_name.clone()),
            )
            .child(self.kind_buttons(cx))
            .child(
                div()
                    .mt_2()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(format!("Title · {count}/{TITLE_LIMIT}")),
            )
            .child(self.outline.clone())
            .child(
                div()
                    .mt_2()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child("Enter adds a point · Tab indents · ⌘Enter saves · Esc keeps draft"),
            )
            .child(div().mt_2().flex().justify_end().child(action_button(
                "worklist-capture-save",
                ActionButton::primary("Save").disabled(!can_save),
                cx,
                cx.listener(|this, _, _, cx| this.submit(cx)),
            )))
            .children(self.error.as_ref().map(|error| {
                div()
                    .mt_2()
                    .type_role(TypeRole::ControlLabel, cx)
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
