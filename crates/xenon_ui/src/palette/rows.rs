//! Shared list-row chrome for palettes (DESIGN multi-channel selection).
//! Callers chain `.on_click(cx.listener(...))` after the row builder.

use gpui::{Div, InteractiveElement, ParentElement, Stateful, Styled, div};
use theme::ThemeColors;

use crate::chrome::list_selection;

/// Single-line row: title only.
pub(crate) fn simple_row(
    id: impl Into<gpui::ElementId>,
    title: String,
    selected: bool,
    colors: &ThemeColors,
) -> Stateful<Div> {
    let paint = list_selection(colors, selected);
    div()
        .id(id)
        .px_3()
        .py_1()
        .text_sm()
        .text_color(paint.foreground)
        .bg(paint.background)
        .font_weight(if selected {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
        .child(title)
}

/// Fields for [`detail_row`] (title + trailing detail + optional subtitle).
pub(crate) struct DetailRow {
    pub title: String,
    pub detail: String,
    pub selected: bool,
    pub selectable: bool,
    pub subtitle: Option<String>,
}

/// Title + trailing detail (+ optional subtitle). Chain `.on_click` when selectable.
pub(crate) fn detail_row(
    id: impl Into<gpui::ElementId>,
    row: DetailRow,
    colors: &ThemeColors,
) -> Stateful<Div> {
    let paint = list_selection(colors, row.selected && row.selectable);
    let muted = colors.text_muted;
    let fg = if row.selectable {
        paint.foreground
    } else {
        muted
    };
    let opacity = if row.selectable { 1.0 } else { 0.45 };
    let mut el = div()
        .id(id)
        .px_3()
        .py_1()
        .flex()
        .flex_col()
        .opacity(opacity)
        .bg(paint.background)
        .child(
            div()
                .flex()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(if row.selected && row.selectable {
                            gpui::FontWeight::MEDIUM
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .text_color(fg)
                        .child(row.title),
                )
                .child(div().text_xs().text_color(muted).child(row.detail)),
        );
    if let Some(sub) = row.subtitle {
        el = el.child(div().text_xs().text_color(muted).child(sub));
    }
    if row.selectable {
        el = el.cursor_pointer().hover(|s| s.bg(colors.element_hover));
    }
    el
}
