//! One selectable row. The caller supplies the content and any extra gestures.

use gpui::{ElementId, InteractiveElement, Stateful, Styled, div, px};
use theme::ThemeColors;

use crate::list_selection;

pub fn selectable_row(
    id: impl Into<ElementId>,
    selected: bool,
    colors: &ThemeColors,
) -> Stateful<gpui::Div> {
    let paint = list_selection(colors, selected);
    let hover = colors.element_hover;
    let text = colors.text;
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap_2()
        .px(px(11.))
        .py(px(10.))
        .rounded_sm()
        .bg(paint.background)
        .text_color(paint.foreground)
        .border_l_2()
        .border_color(paint.accent)
        .cursor_pointer()
        .hover(move |style| style.bg(hover).text_color(text))
}
