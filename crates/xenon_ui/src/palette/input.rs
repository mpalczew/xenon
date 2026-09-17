//! Shared macOS text input registrar + append-query EntityInputHandler.

use gpui::{
    ClipboardItem, Context, Div, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Styled, Window, canvas,
};
use xenon_settings::{Copy, Cut, Paste};

/// Query string owned by an elevated palette. Paste/cut/copy go here, not the tab.
pub(crate) trait QueryBuffer: Sized {
    fn query_text(&self) -> &str;
    fn replace_query(&mut self, query: String, cx: &mut Context<Self>);
}

/// First line of clipboard text for single-line query fields.
pub(crate) fn first_query_line(text: &str) -> Option<String> {
    text.lines()
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

pub(crate) fn clipboard_query_line(item: ClipboardItem) -> Option<String> {
    first_query_line(item.text()?.as_str())
}

pub(crate) fn paste_query<V: QueryBuffer>(this: &mut V, cx: &mut Context<V>) {
    let Some(item) = cx.read_from_clipboard() else {
        return;
    };
    let Some(line) = clipboard_query_line(item) else {
        return;
    };
    let query = format!("{}{line}", this.query_text());
    this.replace_query(query, cx);
}

pub(crate) fn copy_query<V: QueryBuffer>(this: &mut V, cx: &mut Context<V>) {
    let text = this.query_text();
    if text.is_empty() {
        return;
    }
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
}

pub(crate) fn cut_query<V: QueryBuffer>(this: &mut V, cx: &mut Context<V>) {
    copy_query(this, cx);
    this.replace_query(String::new(), cx);
}

pub(crate) struct QueryChrome<V> {
    pub panel: Div,
    pub focus: FocusHandle,
    pub key_context: &'static str,
    pub view: Entity<V>,
}

/// Focus, typed input, and clipboard actions for a palette panel.
pub(crate) fn bind_query_chrome<V>(
    chrome: QueryChrome<V>,
    cx: &mut Context<V>,
    on_key: impl Fn(&mut V, &KeyDownEvent, &mut Window, &mut Context<V>) + 'static,
) -> Div
where
    V: QueryBuffer + EntityInputHandler + 'static,
{
    chrome
        .panel
        .track_focus(&chrome.focus)
        .key_context(chrome.key_context)
        .on_key_down(cx.listener(on_key))
        .on_action(cx.listener(|this, _: &Paste, _, cx| paste_query(this, cx)))
        .on_action(cx.listener(|this, _: &Cut, _, cx| cut_query(this, cx)))
        .on_action(cx.listener(|this, _: &Copy, _, cx| copy_query(this, cx)))
        .child(input_registrar(chrome.view, chrome.focus))
}

/// Transparent full-panel canvas that registers typed text (under results).
pub(crate) fn input_registrar<V>(view: Entity<V>, focus: FocusHandle) -> impl IntoElement
where
    V: EntityInputHandler,
{
    canvas(
        move |_bounds, _window, _cx| {},
        move |bounds, _prepaint, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

/// Noop IME/geometry methods for simple append-style `EntityInputHandler`s.
#[macro_export]
macro_rules! entity_input_noop_geometry {
    () => {
        fn marked_text_range(
            &self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> Option<std::ops::Range<usize>> {
            None
        }

        fn unmark_text(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) {}

        fn bounds_for_range(
            &mut self,
            _range_utf16: std::ops::Range<usize>,
            _element_bounds: gpui::Bounds<gpui::Pixels>,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> Option<gpui::Bounds<gpui::Pixels>> {
            None
        }

        fn character_index_for_point(
            &mut self,
            _point: gpui::Point<gpui::Pixels>,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> Option<usize> {
            None
        }
    };
}

/// Implement query clipboard + `EntityInputHandler` for a palette that owns
/// `query: String` and `fn set_query(&mut self, String, &mut Context<Self>)`.
#[macro_export]
macro_rules! impl_palette_query_input {
    ($ty:ty) => {
        impl $crate::palette::QueryBuffer for $ty {
            fn query_text(&self) -> &str {
                &self.query
            }

            fn replace_query(&mut self, query: String, cx: &mut gpui::Context<Self>) {
                self.set_query(query, cx);
            }
        }

        impl gpui::EntityInputHandler for $ty {
            fn replace_text_in_range(
                &mut self,
                _range: Option<std::ops::Range<usize>>,
                text: &str,
                _window: &mut gpui::Window,
                cx: &mut gpui::Context<Self>,
            ) {
                if !text.is_empty() {
                    let query = format!("{}{}", self.query, text);
                    self.set_query(query, cx);
                }
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                _range: Option<std::ops::Range<usize>>,
                new_text: &str,
                _new_selected_range: Option<std::ops::Range<usize>>,
                _window: &mut gpui::Window,
                cx: &mut gpui::Context<Self>,
            ) {
                if !new_text.is_empty() {
                    let query = format!("{}{}", self.query, new_text);
                    self.set_query(query, cx);
                }
            }

            fn selected_text_range(
                &mut self,
                _ignore_disabled_input: bool,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<Self>,
            ) -> Option<gpui::UTF16Selection> {
                Some(gpui::UTF16Selection {
                    range: 0..0,
                    reversed: false,
                })
            }

            fn text_for_range(
                &mut self,
                _range: std::ops::Range<usize>,
                _adjusted: &mut Option<std::ops::Range<usize>>,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<Self>,
            ) -> Option<String> {
                None
            }

            $crate::entity_input_noop_geometry!();
        }
    };
}

#[cfg(test)]
mod tests {
    use super::first_query_line;

    #[test]
    fn clipboard_line_takes_first_line() {
        assert_eq!(
            first_query_line("api-redesign\nignored").as_deref(),
            Some("api-redesign")
        );
    }

    #[test]
    fn clipboard_line_skips_empty() {
        assert!(first_query_line("").is_none());
        assert_eq!(
            first_query_line("\nonly-newline").as_deref(),
            Some("only-newline")
        );
    }
}
