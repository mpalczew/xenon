use super::TextInputView;
use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use std::ops::Range;

impl EntityInputHandler for TextInputView {
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = if self.config.multiline {
            text.to_owned()
        } else {
            text.replace(['\n', '\r'], " ")
        };
        self.value.replace(range, &text, false);
        self.changed(cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected_range: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = if self.config.multiline {
            text.to_owned()
        } else {
            text.replace(['\n', '\r'], " ")
        };
        self.value.replace(range, &text, true);
        if let Some(selected_range) = selected_range {
            self.value.select_marked_range(selected_range);
        }
        self.changed(cx);
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.value.selected_utf16(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.value.marked_utf16()
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.value.unmark();
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = Some(range.clone());
        Some(self.value.text_for_utf16_range(range))
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.geometry
            .as_ref()
            .map(|geometry| geometry.bounds_for_utf16_range(self.value.text(), range))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.geometry.as_ref().map(|geometry| {
            let index = geometry.index_for_point(point);
            self.value
                .text()
                .chars()
                .take(index)
                .map(char::len_utf16)
                .sum()
        })
    }
}
