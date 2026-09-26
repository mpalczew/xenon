//! Paint one text run, selection, and an overlaid caret without changing text flow.

use gpui::{
    FocusHandle, HighlightStyle, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    StatefulInteractiveElement, Styled, StyledText, div, px,
};
use theme::ThemeColors;

use crate::MultilineText;
use crate::text_input::TextInputAppearance;

pub(crate) struct FieldChrome<'a> {
    pub value: &'a MultilineText,
    pub placeholder: &'a str,
    pub height: Pixels,
    pub colors: &'a ThemeColors,
    pub focus: FocusHandle,
    pub caret: Option<Point<Pixels>>,
    pub line_height: Pixels,
    pub appearance: TextInputAppearance,
}

pub(crate) fn text_field(field: FieldChrome<'_>) -> impl IntoElement + use<> {
    let FieldChrome {
        value,
        placeholder,
        height,
        colors,
        focus,
        caret,
        line_height,
        appearance,
    } = field;
    let text = if value.text().is_empty() {
        div()
            .text_color(colors.text_muted)
            .child(placeholder.to_owned())
            .into_any_element()
    } else {
        let selection = value.selected_byte_range();
        let styled = StyledText::new(value.text().to_owned());
        let styled = if selection.is_empty() {
            styled
        } else {
            styled.with_highlights([(
                selection,
                HighlightStyle {
                    background_color: Some(colors.element_selected),
                    ..Default::default()
                },
            )])
        };
        div()
            .text_color(colors.text)
            .child(styled)
            .into_any_element()
    };
    let base = div()
        .id("text-input-field")
        .relative()
        .min_h(height)
        .child(text)
        .children(caret.map(|point| {
            div()
                .absolute()
                .left(point.x)
                .top(point.y)
                .w(px(1.))
                .h(line_height)
                .bg(colors.text)
        }))
        .on_click(move |_, window, cx| focus.focus(window, cx));
    match appearance {
        TextInputAppearance::Bordered => base
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(colors.border_focused)
            .bg(colors.editor_background)
            .into_any_element(),
        TextInputAppearance::Palette => base
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .into_any_element(),
        TextInputAppearance::Inline => base.into_any_element(),
    }
}
