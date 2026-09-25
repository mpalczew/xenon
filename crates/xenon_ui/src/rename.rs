//! Workspace rename composes the shared single-line text control.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div,
};
use theme::ActiveTheme;
use xenon_design_system::{TextInputAppearance, TextInputConfig, TextInputEvent, TextInputView};

pub enum RenameEvent {
    Committed(String),
    Cancelled,
}

pub struct RenameView {
    input: Entity<TextInputView>,
    finished: bool,
    _input_sub: Subscription,
    _blur: Option<Subscription>,
}

impl EventEmitter<RenameEvent> for RenameView {}

impl RenameView {
    pub fn new(initial: String, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line("Name").appearance(TextInputAppearance::Inline),
                cx,
            )
        });
        input.update(cx, |input, cx| {
            input.set_text(initial, cx);
            input.open(cx);
        });
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            TextInputEvent::Changed(_) => {}
            TextInputEvent::Submit(_) => this.commit_or_cancel(cx),
            TextInputEvent::Cancel => this.finish(RenameEvent::Cancelled, cx),
        });
        Self {
            input,
            finished: false,
            _input_sub: subscription,
            _blur: None,
        }
    }

    fn finish(&mut self, event: RenameEvent, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        cx.emit(event);
    }

    fn commit_or_cancel(&mut self, cx: &mut Context<Self>) {
        let name = self.input.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.finish(RenameEvent::Cancelled, cx);
        } else {
            self.finish(RenameEvent::Committed(name), cx);
        }
    }
}

impl Focusable for RenameView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle()
    }
}

impl Render for RenameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self._blur.is_none() {
            let focus = self.input.read(cx).focus_handle();
            self._blur = Some(cx.on_blur(&focus, window, |this, _, cx| {
                this.commit_or_cancel(cx);
            }));
        }
        let colors = cx.theme().colors().clone();
        div()
            .w_full()
            .px_1()
            .text_sm()
            .rounded_sm()
            .bg(colors.editor_background)
            .border_1()
            .border_color(colors.border_focused)
            .child(self.input.clone())
    }
}
