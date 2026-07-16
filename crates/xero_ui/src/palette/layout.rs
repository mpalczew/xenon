//! Scrim, panel, query, hint, and scrollable results — one geometry for all
//! elevated palettes (DESIGN.md elevation 2).

use gpui::{
    AnyElement, Div, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};
use nucleo::Matcher;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use theme::ThemeColors;

/// Default palette panel geometry.
#[derive(Clone, Copy)]
pub(crate) struct PaletteLayout {
    pub width: f32,
    pub max_h: f32,
    pub top: f32,
}

impl Default for PaletteLayout {
    fn default() -> Self {
        Self {
            width: 640.,
            max_h: 420.,
            top: 80.,
        }
    }
}

impl PaletteLayout {
    pub(crate) fn tall() -> Self {
        Self {
            max_h: 480.,
            top: 72.,
            ..Self::default()
        }
    }
}

/// Outer dismiss scrim (caller attaches `.on_click` for dismiss).
pub(crate) fn scrim(scrim_id: &'static str, layout: PaletteLayout) -> Stateful<Div> {
    div()
        .id(scrim_id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(layout.top))
}

/// Elevated panel shell (caller attaches focus, key_context, on_key_down).
/// Returns plain `Div` so `.track_focus` / `.key_context` chain cleanly.
pub(crate) fn panel(layout: PaletteLayout, colors: &ThemeColors) -> Div {
    div()
        .occlude()
        .relative()
        .w(px(layout.width))
        .max_h(px(layout.max_h))
        .flex()
        .flex_col()
        .min_h_0()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
}

pub(crate) fn query_row(query: &str, placeholder: &str, colors: &ThemeColors) -> Div {
    let empty = query.is_empty();
    let shown = if empty {
        placeholder.to_string()
    } else {
        query.to_string()
    };
    div()
        .flex_none()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(colors.border)
        .text_color(if empty {
            colors.text_muted
        } else {
            colors.text
        })
        .child(shown)
}

pub(crate) fn optional_title(title: &str, colors: &ThemeColors) -> Div {
    div()
        .flex_none()
        .px_3()
        .pt_2()
        .text_xs()
        .text_color(colors.text_muted)
        .child(title.to_string())
}

pub(crate) fn hint_row(text: &str, colors: &ThemeColors) -> Div {
    div()
        .flex_none()
        .px_3()
        .py_1()
        .border_t_1()
        .border_color(colors.border)
        .text_xs()
        .text_color(colors.text_muted)
        .child(text.to_string())
}

/// Hint bar with a trailing action (e.g. Browse…).
pub(crate) fn hint_row_with_action(text: &str, action: AnyElement, colors: &ThemeColors) -> Div {
    div()
        .flex_none()
        .px_3()
        .py_1()
        .border_t_1()
        .border_color(colors.border)
        .flex()
        .justify_between()
        .text_xs()
        .text_color(colors.text_muted)
        .child(text.to_string())
        .child(action)
}

/// Scrollable result list (or empty message). Always flex_1 so the panel cap works.
pub(crate) fn scroll_results(
    list_id: &'static str,
    empty_message: &str,
    rows: Vec<AnyElement>,
    colors: &ThemeColors,
) -> AnyElement {
    if rows.is_empty() {
        return div()
            .id(list_id)
            .flex_1()
            .min_h_0()
            .px_3()
            .py_2()
            .text_sm()
            .text_color(colors.text_muted)
            .child(empty_message.to_string())
            .into_any_element();
    }
    div()
        .id(list_id)
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .children(rows)
        .into_any_element()
}

/// Clamp selection after ±delta. No-op on empty.
pub(crate) fn clamp_selection(selected: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = (len - 1) as isize;
    (selected as isize + delta).clamp(0, last) as usize
}

/// Fuzzy-rank indices of `haystacks` for `query` (empty → identity order).
pub(crate) fn fuzzy_index_order(
    haystacks: &[String],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<usize> {
    if query.is_empty() {
        return (0..haystacks.len()).collect();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let labels: Vec<&str> = haystacks.iter().map(|s| s.as_str()).collect();
    let mut scored: Vec<(usize, u32)> = pattern
        .match_list(labels.iter().copied(), matcher)
        .into_iter()
        .filter_map(|(label, score)| {
            haystacks
                .iter()
                .position(|h| h.as_str() == label)
                .map(|i| (i, score))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, _)| i).collect()
}
