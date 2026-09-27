//! One row in a query palette: type roles, accent edge, match runs, detail chip.

use std::ops::Range;

use gpui::{
    App, ElementId, FontWeight, HighlightStyle, InteractiveElement, IntoElement, ParentElement,
    Stateful, Styled, StyledText, div, hsla, px,
};
use theme::ActiveTheme;

use crate::{TypeRole, Typography};

pub struct QueryRow {
    pub title: String,
    pub detail: Option<String>,
    pub subtitle: Option<String>,
    pub selected: bool,
    pub enabled: bool,
    /// Char indexes into `title` painted in the accent.
    pub hits: Vec<u32>,
}

pub fn query_row(id: impl Into<ElementId>, row: QueryRow, cx: &App) -> Stateful<gpui::Div> {
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    let hover = colors.element_hover;
    let selected_fill = colors.element_selected;
    let edge = if row.selected {
        accent
    } else {
        hsla(0., 0., 0., 0.)
    };
    let mut el = div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(1.))
        .px_3()
        .py(px(6.))
        .border_l(px(2.))
        .border_color(edge)
        .cursor_pointer();
    if row.selected {
        el = el.bg(selected_fill);
    } else if row.enabled {
        el = el.hover(move |style| style.bg(hover));
    }
    if !row.enabled {
        el = el.opacity(0.45);
    }
    let mut title = div().min_w_0();
    if row.selected && row.enabled {
        title = title.type_role(TypeRole::ListPrimary, cx);
    } else {
        title = title
            .type_role(TypeRole::Body, cx)
            .text_color(colors.text_muted);
    }
    title = title.child(title_runs(&row.title, &row.hits, accent));
    let mut top = div()
        .flex()
        .justify_between()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(title);
    if let Some(detail) = row.detail.filter(|detail| !detail.is_empty()) {
        top = top.child(
            div()
                .flex_none()
                .px(px(6.))
                .rounded(px(4.))
                .border_1()
                .border_color(colors.border)
                .type_role(TypeRole::Code, cx)
                .child(detail),
        );
    }
    el = el.child(top);
    if let Some(subtitle) = row.subtitle.filter(|subtitle| !subtitle.is_empty()) {
        el = el.child(div().type_role(TypeRole::ControlLabel, cx).child(subtitle));
    }
    el
}

pub fn query_label(text: &str, cx: &App) -> gpui::Div {
    div()
        .flex_none()
        .px_3()
        .pt_2()
        .type_role(TypeRole::ControlLabel, cx)
        .child(text.to_string())
}

pub fn query_hint(text: &str, cx: &App) -> gpui::Div {
    let border = cx.theme().colors().border;
    div()
        .flex_none()
        .px_3()
        .py_1()
        .border_t_1()
        .border_color(border)
        .type_role(TypeRole::ControlLabel, cx)
        .child(text.to_string())
}

pub fn query_hint_action(text: &str, action: impl IntoElement, cx: &App) -> gpui::Div {
    let border = cx.theme().colors().border;
    div()
        .flex_none()
        .px_3()
        .py_1()
        .border_t_1()
        .border_color(border)
        .flex()
        .justify_between()
        .items_center()
        .type_role(TypeRole::ControlLabel, cx)
        .child(text.to_string())
        .child(action)
}

fn title_runs(title: &str, hits: &[u32], accent: gpui::Hsla) -> gpui::AnyElement {
    let ranges = hit_ranges(title, hits);
    if ranges.is_empty() {
        return title.to_string().into_any_element();
    }
    let style = HighlightStyle {
        color: Some(accent),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(title.to_string())
        .with_highlights(ranges.into_iter().map(|range| (range, style)))
        .into_any_element()
}

fn hit_ranges(text: &str, hits: &[u32]) -> Vec<Range<usize>> {
    let mut ends: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    ends.push(text.len());
    let mut sorted: Vec<usize> = hits
        .iter()
        .map(|index| *index as usize)
        .filter(|index| index + 1 < ends.len())
        .collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < sorted.len() {
        let start = sorted[cursor];
        let mut end = start;
        while cursor + 1 < sorted.len() && sorted[cursor + 1] == end + 1 {
            cursor += 1;
            end = sorted[cursor];
        }
        ranges.push(ends[start]..ends[end + 1]);
        cursor += 1;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_hits_are_one_run() {
        assert_eq!(hit_ranges("src/main.rs", &[4, 5, 6, 7]), vec![4..8]);
    }

    #[test]
    fn a_gap_splits_runs() {
        assert_eq!(hit_ranges("ab_cd", &[0, 3, 4]), vec![0..1, 3..5]);
    }
}
