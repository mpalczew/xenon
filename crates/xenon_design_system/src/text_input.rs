//! A complete native text control: input, focus, editing, and presentation.

use gpui::{
    App, ClipboardItem, Context, ElementInputHandler, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render, Styled,
    Window, canvas, div, point, px,
};
use theme::ActiveTheme;
use xenon_settings::{Copy, Cut, Paste};

use crate::text_field::{FieldChrome, text_field};
use crate::{CursorBlink, FocusOnOpen, MultilineText};

mod config;
mod geometry;
mod history;
mod input;
pub use config::{TextInputAppearance, TextInputConfig, TextInputKeyBehavior};

pub enum TextInputEvent {
    Changed(String),
    Submit(String),
    Cancel,
    /// A navigation key the field did not edit, for a parent such as an outline.
    ParentKey {
        key: String,
        shift: bool,
        platform: bool,
    },
}

pub struct TextInputView {
    value: MultilineText,
    config: TextInputConfig,
    focus: FocusHandle,
    focus_on_open: FocusOnOpen,
    geometry: Option<geometry::TextGeometry>,
    blink: CursorBlink,
    history: history::History,
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
            blink: CursorBlink::default(),
            history: history::History::default(),
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
        self.history.clear();
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

    pub(crate) fn split_off_suffix(&mut self, cx: &mut Context<Self>) -> String {
        let before = self.value.points();
        let suffix = self.value.split_off_suffix();
        if before.0 != self.value.text() {
            self.history.record(before);
        }
        self.changed(cx);
        suffix
    }

    pub(crate) fn place_caret(&mut self, index: usize, cx: &mut Context<Self>) {
        self.value.set_caret(index);
        cx.notify();
    }

    /// Focus this field and put the caret on the character closest to a window point,
    /// including clicks in the gutter or past the end of the line.
    pub(crate) fn place_at(
        &mut self,
        position: Point<Pixels>,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        self.blink.reset(cx, Self::blink_tick);
        let index = self
            .geometry
            .as_ref()
            .map(|geometry| geometry.index_for_point(position))
            .unwrap_or(0);
        self.value.place_caret(index, extend);
        cx.notify();
    }

    pub(crate) fn vertical_distance(&self, y: Pixels) -> Pixels {
        let Some(bounds) = self.geometry.as_ref().map(geometry::TextGeometry::bounds) else {
            return px(f32::MAX);
        };
        if y < bounds.top() {
            bounds.top() - y
        } else if y > bounds.bottom() {
            y - bounds.bottom()
        } else {
            px(0.)
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Changed(self.value.text().to_owned()));
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.blink.reset(cx, Self::blink_tick);
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if modifiers.platform && !modifiers.control && key == "z" {
            let restored = if modifiers.shift {
                self.history.redo(&mut self.value)
            } else {
                self.history.undo(&mut self.value)
            };
            if restored {
                self.changed(cx);
            } else {
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if key == "enter" && self.value.marked_utf16().is_some() {
            self.value.unmark();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if matches!(
            self.config.key_behavior,
            TextInputKeyBehavior::ParentNavigation
        ) && (matches!(key, "enter" | "escape" | "tab" | "up" | "down")
            || (key == "backspace" && (modifiers.platform || self.value.is_caret_at_start())))
        {
            cx.emit(TextInputEvent::ParentKey {
                key: key.to_owned(),
                shift: modifiers.shift,
                platform: modifiers.platform,
            });
            cx.stop_propagation();
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
        let before = self.value.points();
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
            if before.0 != self.value.text() {
                self.history.record(before);
            }
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
            let before = self.value.points();
            self.value.replace(None, "", false);
            self.history.record(before);
            self.changed(cx);
            self.blink.reset(cx, Self::blink_tick);
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = if self.config.multiline {
                text
            } else {
                text.replace(['\n', '\r'], " ")
            };
            let before = self.value.points();
            self.value.replace(None, &text, false);
            if before.0 != self.value.text() {
                self.history.record(before);
            }
            self.changed(cx);
            self.blink.reset(cx, Self::blink_tick);
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
        self.blink.reset(cx, Self::blink_tick);
        if let Some(geometry) = &self.geometry {
            self.value.place_caret(
                geometry.index_for_point(event.position),
                event.modifiers.shift,
            );
            cx.notify();
        }
    }

    fn blink_tick(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        let active = self.blink.tick(generation);
        if active {
            cx.notify();
        }
        active
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
        let focused = self.focus.is_focused(window)
            && (window.is_window_active() || crate::motion_frozen(cx));
        self.blink.update_focus(focused, cx, Self::blink_tick);
        if let Some(bounds) = self.geometry.as_ref().map(geometry::TextGeometry::bounds) {
            self.geometry = Some(self.geometry_for_bounds(bounds, window));
        }
        let colors = cx.theme().colors().clone();
        let caret =
            (focused && self.blink.visible() && self.value.selection().is_empty()).then(|| {
                let utf16 = self.value.selected_utf16().end;
                self.geometry.as_ref().map_or_else(
                    || {
                        let (x, y) =
                            geometry::field_insets(self.config.appearance, window.rem_size());
                        point(x, y)
                    },
                    |geometry| geometry.caret_offset(self.value.text(), utf16),
                )
            });
        let field = text_field(FieldChrome {
            value: &self.value,
            placeholder: &self.config.placeholder,
            height: self.config.min_height,
            colors: &colors,
            focus: self.focus.clone(),
            caret,
            line_height: window.line_height(),
            appearance: self.config.appearance,
        });
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
