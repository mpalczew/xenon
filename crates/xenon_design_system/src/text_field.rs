//! Reusable multiline field chrome and macOS input registration.

use gpui::{
    FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, div,
};
use theme::ThemeColors;

use crate::MultilineText;

pub(crate) struct FieldChrome<'a> {
    pub value: &'a MultilineText,
    pub placeholder: &'a str,
    pub height: Pixels,
    pub colors: &'a ThemeColors,
    pub focus: FocusHandle,
    pub focused: bool,
}

pub(crate) fn multiline_field_with_caret(field: FieldChrome<'_>) -> impl IntoElement + use<> {
    let FieldChrome {
        value,
        placeholder,
        height,
        colors,
        focus,
        focused,
    } = field;
    let content = if value.text().is_empty() {
        div()
            .flex()
            .items_baseline()
            .text_color(colors.text_muted)
            .children(focused.then(|| div().text_color(colors.text).child("│")))
            .child(placeholder.to_string())
    } else {
        let (before, selected, after) = value.split_at_caret();
        div()
            .text_color(colors.text)
            .child(before)
            .children(
                (!selected.is_empty()).then(|| div().bg(colors.element_selected).child(selected)),
            )
            .children(focused.then_some("│"))
            .child(after)
    };
    div()
        .id("multiline-field")
        .min_h(height)
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(colors.border_focused)
        .bg(colors.editor_background)
        .child(content)
        .on_click(move |_, window, cx| focus.focus(window, cx))
}
