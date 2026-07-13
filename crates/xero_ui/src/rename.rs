//! `RenameView`: a one-line inline text field for renaming a stream or
//! workspace. Emits `RenameEvent` back to `XeroApp`. Same `EntityInputHandler`
//! + registrar-canvas pattern as the finder/editor/terminal.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Point, Render, Styled, Subscription, Task, UTF16Selection, Window, canvas, div, px,
};
use theme::ActiveTheme;

const CARET_BLINK: Duration = Duration::from_millis(530);

pub enum RenameEvent {
    Committed(String),
    Cancelled,
}

pub struct RenameView {
    text: String,
    focus: FocusHandle,
    focused_once: bool,
    caret_on: bool,
    /// Prevent double-emit when blur fires after Escape/Enter already finished.
    finished: bool,
    _blur: Option<Subscription>,
    _blink: Option<Task<()>>,
}

impl EventEmitter<RenameEvent> for RenameView {}

impl RenameView {
    pub fn new(initial: String, cx: &mut Context<Self>) -> Self {
        Self {
            text: initial,
            focus: cx.focus_handle(),
            focused_once: false,
            caret_on: true,
            finished: false,
            _blur: None,
            _blink: None,
        }
    }

    fn finish(&mut self, event: RenameEvent, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        self._blink = None;
        cx.emit(event);
    }

    fn commit_or_cancel(&mut self, cx: &mut Context<Self>) {
        let name = self.text.trim().to_string();
        if name.is_empty() {
            self.finish(RenameEvent::Cancelled, cx);
        } else {
            self.finish(RenameEvent::Committed(name), cx);
        }
    }

    fn start_caret_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        self._blink = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CARET_BLINK).await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.finished {
                            this._blink = None;
                            return false;
                        }
                        this.caret_on = !this.caret_on;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => self.finish(RenameEvent::Cancelled, cx),
            "enter" => self.commit_or_cancel(cx),
            "backspace" => {
                self.text.pop();
                self.caret_on = true;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Focusable for RenameView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RenameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
            self.start_caret_blink(cx);
        }
        // Click-away (or focus move to terminal/editor) must leave edit mode.
        if self._blur.is_none() {
            let focus = self.focus.clone();
            self._blur = Some(cx.on_blur(&focus, window, |this, _window, cx| {
                this.commit_or_cancel(cx);
            }));
        }
        let colors = cx.theme().colors().clone();
        let caret = self
            .caret_on
            .then(|| div().w(px(1.)).h(px(14.)).flex_none().bg(colors.text));
        div()
            .track_focus(&self.focus)
            .key_context("Rename")
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .w_full()
            .px_1()
            .text_sm()
            .rounded_sm()
            .bg(colors.editor_background)
            .border_1()
            .border_color(colors.border_focused)
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_h(px(18.))
                    .child(div().truncate().child(self.text.clone()))
                    .children(caret),
            )
            .child(input_registrar(cx.entity(), self.focus.clone()))
    }
}

/// A transparent full-size canvas that registers the text input handler during
/// paint (required to receive typed characters on macOS).
fn input_registrar(view: Entity<RenameView>, focus: FocusHandle) -> impl IntoElement {
    canvas(
        move |_bounds, _window, _cx| {},
        move |bounds, _prepaint, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

impl EntityInputHandler for RenameView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text.push_str(text);
        self.caret_on = true;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text.push_str(new_text);
        self.caret_on = true;
        cx.notify();
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        // Caret at end of text (UTF-16 offsets for the platform input system).
        let n = self.text.encode_utf16().count();
        Some(UTF16Selection {
            range: n..n,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
