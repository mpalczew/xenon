//! A complete native text control: input, focus, editing, and presentation.

use gpui::{
    App, ClipboardItem, Context, ElementInputHandler, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, canvas, div,
};
use theme::ActiveTheme;
use xenon_settings::{Copy, Cut, Paste};

use crate::text_field::{FieldChrome, multiline_field_with_caret};
use crate::{FocusOnOpen, MultilineText};

mod config;
mod geometry;
mod input;
pub use config::{TextInputAppearance, TextInputConfig, TextInputKeyBehavior};

pub enum TextInputEvent {
    Changed(String),
    Submit(String),
    Cancel,
}

pub struct TextInputView {
    value: MultilineText,
    config: TextInputConfig,
    focus: FocusHandle,
    focus_on_open: FocusOnOpen,
    geometry: Option<geometry::TextGeometry>,
}

impl EventEmitter<TextInputEvent> for TextInputView {}

impl TextInputView {
    pub fn new(config: TextInputConfig, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        Self {
            value: MultilineText::default(),
            config,
            focus_on_open: FocusOnOpen::new(focus.clone()),
            focus,
            geometry: None,
        }
    }

    pub fn text(&self) -> &str {
        self.value.text()
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        if self.value.text() == text {
            return;
        }
        self.value = MultilineText::new(text);
        cx.notify();
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>, cx: &mut Context<Self>) {
        let placeholder = placeholder.into();
        if self.config.placeholder == placeholder {
            return;
        }
        self.config.placeholder = placeholder;
        cx.notify();
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.focus_on_open.open();
        cx.notify();
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Changed(self.value.text().to_owned()));
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if key == "enter" && self.value.marked_utf16().is_some() {
            self.value.unmark();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if matches!(
            self.config.key_behavior,
            TextInputKeyBehavior::ParentNavigation
        ) && matches!(key, "enter" | "escape" | "tab" | "up" | "down")
        {
            return;
        }
        if matches!(self.config.key_behavior, TextInputKeyBehavior::DialogField)
            && matches!(key, "enter" | "escape" | "tab")
        {
            return;
        }
        if modifiers.platform && key == "a" {
            self.value.select_all();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        let changed = match key {
            "escape" => {
                cx.emit(TextInputEvent::Cancel);
                false
            }
            "enter"
                if self.config.multiline
                    && matches!(
                        self.config.key_behavior,
                        TextInputKeyBehavior::SubmitOnPlainEnter
                    )
                    && modifiers.shift =>
            {
                self.value.replace(None, "\n", false);
                true
            }
            "enter"
                if self.config.multiline
                    && matches!(
                        self.config.key_behavior,
                        TextInputKeyBehavior::SubmitAndCancel
                    )
                    && !modifiers.platform =>
            {
                self.value.replace(None, "\n", false);
                true
            }
            "enter" => {
                cx.emit(TextInputEvent::Submit(self.value.text().to_owned()));
                false
            }
            _ => match self.edit_key(key, modifiers.shift) {
                Some(changed) => changed,
                None => return,
            },
        };
        if changed {
            self.changed(cx);
        } else {
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn edit_key(&mut self, key: &str, extend: bool) -> Option<bool> {
        match key {
            "backspace" => {
                self.value.backspace();
                Some(true)
            }
            "delete" => {
                self.value.delete();
                Some(true)
            }
            "left" => {
                self.value.left(extend);
                Some(false)
            }
            "right" => {
                self.value.right(extend);
                Some(false)
            }
            "home" => {
                self.value.home(extend);
                Some(false)
            }
            "end" => {
                self.value.end(extend);
                Some(false)
            }
            "up" => {
                if self.config.multiline {
                    self.value.up(extend);
                } else {
                    self.value.home(extend);
                }
                Some(false)
            }
            "down" => {
                if self.config.multiline {
                    self.value.down(extend);
                } else {
                    self.value.end(extend);
                }
                Some(false)
            }
            _ => None,
        }
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let selected = self.value.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        let selected = self.value.selected_text().to_owned();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected));
            self.value.replace(None, "", false);
            self.changed(cx);
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = if self.config.multiline {
                text
            } else {
                text.replace(['\n', '\r'], " ")
            };
            self.value.replace(None, &text, false);
            self.changed(cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != gpui::MouseButton::Left {
            return;
        }
        self.focus.focus(window, cx);
        if let Some(geometry) = &self.geometry {
            self.value.place_caret(
                geometry.index_for_point(event.position),
                event.modifiers.shift,
            );
            cx.notify();
        }
    }
}

impl Focusable for TextInputView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextInputView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focus_on_open.focus_after_open(window, cx);
        let colors = cx.theme().colors().clone();
        let field = match self.config.appearance {
            TextInputAppearance::Bordered => multiline_field_with_caret(FieldChrome {
                value: &self.value,
                placeholder: &self.config.placeholder,
                height: self.config.min_height,
                colors: &colors,
                focus: self.focus.clone(),
                focused: self.focus.is_focused(window),
            })
            .into_any_element(),
            TextInputAppearance::Inline | TextInputAppearance::Palette => {
                let content = if self.value.text().is_empty() {
                    div()
                        .flex()
                        .items_baseline()
                        .text_color(colors.text_muted)
                        .children(
                            self.focus
                                .is_focused(window)
                                .then(|| div().text_color(colors.text).child("│")),
                        )
                        .child(self.config.placeholder.clone())
                } else {
                    let (before, selected, after) = self.value.split_at_caret();
                    div()
                        .text_color(colors.text)
                        .child(before)
                        .children(
                            (!selected.is_empty())
                                .then(|| div().bg(colors.element_selected).child(selected)),
                        )
                        .children(self.focus.is_focused(window).then_some("│"))
                        .child(after)
                };
                let field = div()
                    .id("text-input-inline")
                    .min_h(self.config.min_height)
                    .child(content)
                    .on_click({
                        let focus = self.focus.clone();
                        move |_, window, cx| focus.focus(window, cx)
                    });
                if matches!(self.config.appearance, TextInputAppearance::Palette) {
                    field
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(colors.border)
                        .into_any_element()
                } else {
                    field.into_any_element()
                }
            }
        };
        div()
            .id("text-input")
            .relative()
            .child(field)
            .child(input_host(cx.entity(), self.focus.clone()))
            .track_focus(&self.focus)
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &Cut, _, cx| this.cut(cx)))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste(cx)))
    }
}

fn input_host(view: Entity<TextInputView>, focus: FocusHandle) -> impl IntoElement {
    canvas(
        move |_, _, _| {},
        move |bounds, _, window, cx| {
            let geometry = view.read(cx).geometry_for_bounds(bounds, window);
            view.update(cx, |input, _| input.geometry = Some(geometry));
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}
