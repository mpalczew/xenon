//! Paint one text run, selection, and an overlaid caret without changing text flow.

use gpui::{
    FocusHandle, HighlightStyle, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    Point, StatefulInteractiveElement, Styled, StyledText, div, px,
};
use std::ops::Range;
use theme::ThemeColors;
use unicode_segmentation::UnicodeSegmentation;

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
    /// Soft grapheme limit and the color that tints text past it.
    pub overflow: Option<(usize, Hsla)>,
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
        overflow,
    } = field;
    let text = if value.text().is_empty() {
        div()
            .text_color(colors.text_muted)
            .child(placeholder.to_owned())
            .into_any_element()
    } else {
        let text = value.text();
        let over = overflow.and_then(|(limit, _)| overflow_range(text, limit));
        let styles = highlights(value.selected_byte_range(), over)
            .into_iter()
            .map(|(range, tint)| {
                let background = match tint {
                    Tint::Selection => colors.element_selected,
                    Tint::Overflow => {
                        overflow.map_or(colors.element_selected, |(_, c)| c.opacity(0.22))
                    }
                };
                let style = HighlightStyle {
                    background_color: Some(background),
                    ..Default::default()
                };
                (range, style)
            });
        let styled = StyledText::new(text.to_owned()).with_highlights(styles);
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

enum Tint {
    Selection,
    Overflow,
}

/// Byte range of the graphemes past `limit`.
fn overflow_range(text: &str, limit: usize) -> Option<Range<usize>> {
    text.grapheme_indices(true)
        .nth(limit)
        .map(|(start, _)| start..text.len())
}

/// Sorted, non-overlapping highlights; the selection wins where they meet.
fn highlights(
    selection: Range<usize>,
    overflow: Option<Range<usize>>,
) -> Vec<(Range<usize>, Tint)> {
    let mut out = Vec::new();
    if let Some(over) = overflow {
        let (cut_start, cut_end) = if selection.is_empty() {
            (over.end, over.end)
        } else {
            (selection.start, selection.end)
        };
        out.push((over.start..over.end.min(cut_start), Tint::Overflow));
        out.push((over.start.max(cut_end)..over.end, Tint::Overflow));
    }
    if !selection.is_empty() {
        out.push((selection, Tint::Selection));
    }
    out.retain(|(range, _)| range.start < range.end);
    out.sort_by_key(|(range, _)| range.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(selection: Range<usize>, overflow: Option<Range<usize>>) -> Vec<(Range<usize>, bool)> {
        highlights(selection, overflow)
            .into_iter()
            .map(|(range, tint)| (range, matches!(tint, Tint::Selection)))
            .collect()
    }

    #[test]
    fn overflow_starts_after_the_limit_in_graphemes() {
        assert_eq!(overflow_range("abcd", 4), None);
        assert_eq!(overflow_range("abcde", 4), Some(4..5));
        assert_eq!(overflow_range("e\u{301}e\u{301}x", 2), Some(6..7));
    }

    #[test]
    fn selection_splits_the_overflow_without_overlap() {
        assert_eq!(spans(3..3, Some(5..10)), vec![(5..10, false)]);
        assert_eq!(
            spans(6..8, Some(5..10)),
            vec![(5..6, false), (6..8, true), (8..10, false)]
        );
        assert_eq!(spans(0..7, Some(5..10)), vec![(0..7, true), (7..10, false)]);
        assert_eq!(spans(1..2, None), vec![(1..2, true)]);
    }
}
