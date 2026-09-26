//! Shared list-row chrome for palettes (DESIGN multi-channel selection).
//! Callers chain `.on_click(cx.listener(...))` after the row builder.

use gpui::{Div, ParentElement, Stateful, Styled, div};
use theme::ThemeColors;

use xenon_design_system::selectable_row;

/// Single-line row: title only.
pub(crate) fn simple_row(
    id: impl Into<gpui::ElementId>,
    title: String,
    selected: bool,
    colors: &ThemeColors,
) -> Stateful<Div> {
    selectable_row(id, selected, colors).py_1().child(title)
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
    let muted = colors.text_muted;
    let opacity = if row.selectable { 1.0 } else { 0.45 };
    let mut el = selectable_row(id, row.selected && row.selectable, colors)
        .py_1()
        .flex_col()
        .items_stretch()
        .opacity(opacity)
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
                        .child(row.title),
                )
                .child(div().text_xs().text_color(muted).child(row.detail)),
        );
    if let Some(sub) = row.subtitle {
        el = el.child(div().text_xs().text_color(muted).child(sub));
    }
    el
}
