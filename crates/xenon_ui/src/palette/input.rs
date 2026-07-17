//! Shared macOS text input registrar + append-query EntityInputHandler.

use gpui::{
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, IntoElement, Styled, canvas,
};

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

/// Implement `EntityInputHandler` for a palette that owns `query: String` and
/// `fn set_query(&mut self, String, &mut Context<Self>)`.
#[macro_export]
macro_rules! impl_palette_query_input {
    ($ty:ty) => {
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
