use gpui::{Bounds, Pixels, Window, point, px, size};

use super::{EditorLayout, HitLayout, LayoutInput, TextMetrics, VisibleRows};

pub(super) struct LayoutSizes {
    pub cell: Pixels,
    pub text_width: Pixels,
    pub content_width: Pixels,
    pub content_height: Pixels,
}

pub(super) fn scroll_offsets(
    input: &LayoutInput<'_>,
    metrics: &TextMetrics<'_>,
    sizes: &LayoutSizes,
) -> (Pixels, Pixels) {
    let (mut top, mut left) = (input.scroll_top, input.scroll_left);
    let (width, height) = crate::scroll::follow_viewport(
        sizes.text_width,
        input.viewport_height,
        sizes.content_width,
        sizes.content_height,
        crate::scroll::scrollbar_reserve(),
    );
    if input.center_cursor {
        top = crate::scroll::center_row(input.cursor.0, metrics.line_height, height);
        left = crate::scroll::keep_col_visible(left, input.cursor.1, sizes.cell, width);
    } else if input.follow_cursor {
        top = crate::scroll::keep_row_visible(top, input.cursor.0, metrics.line_height, height);
        left = crate::scroll::keep_col_visible(left, input.cursor.1, sizes.cell, width);
    }
    top = top
        .min((sizes.content_height - input.viewport_height).max(px(0.)))
        .max(px(0.));
    left = left
        .min((sizes.content_width - sizes.text_width).max(px(0.)))
        .max(px(0.));
    (top, left)
}

pub(super) fn layout_plain(
    input: LayoutInput<'_>,
    metrics: TextMetrics<'_>,
    window: &mut Window,
) -> EditorLayout {
    let cell_w = super::cell_width(window, metrics.font, metrics.font_size);
    let gutter_width = super::gutter_width(input.rope.len_lines(), cell_w, input.show_line_numbers);
    let text_width = (input.viewport_width - gutter_width).max(px(0.));
    let content_width = super::content_width(input.rope, cell_w);
    let content_height = metrics.line_height * (input.rope.len_lines() as f32);
    let (row, col) = input.cursor;
    let sizes = LayoutSizes {
        cell: cell_w,
        text_width,
        content_width,
        content_height,
    };
    let (scroll_top, scroll_left) = scroll_offsets(&input, &metrics, &sizes);
    let text_origin = point(input.origin.x + gutter_width - scroll_left, input.origin.y);
    let gutter = input
        .show_line_numbers
        .then(|| Bounds::new(input.origin, size(gutter_width, input.viewport_height)));
    let scrollbars = crate::scroll::scrollbars(crate::scroll::ScrollbarInput {
        origin: input.origin,
        viewport_width: input.viewport_width,
        viewport_height: input.viewport_height,
        gutter_width,
        text_width,
        content_width,
        content_height,
        scroll_top,
        scroll_left,
    });
    let total = input.rope.len_lines();
    let first = (f32::from(scroll_top) / f32::from(metrics.line_height))
        .floor()
        .max(0.) as usize;
    let visible =
        (f32::from(input.viewport_height) / f32::from(metrics.line_height)).ceil() as usize + 1;
    let last = (first + visible).min(total);
    let (lines, line_numbers) =
        super::shape_visible_lines(&input, &metrics, VisibleRows { first, last, total }, window);
    let cursor = Bounds::new(
        point(
            text_origin.x + cell_w * (col as f32),
            input.origin.y + metrics.line_height * (row as f32) - scroll_top,
        ),
        size(cell_w, metrics.line_height),
    );
    let hits = HitLayout {
        rope: input.rope,
        text_origin,
        origin_y: input.origin.y,
        cell_w,
        line_height: metrics.line_height,
        scroll_top,
        first_row: first,
        last_row: last,
        wrap_rows: None,
    };
    let mut selection = Vec::new();
    for range in input.selection_ranges {
        selection.extend(hits.rects(Some(range)));
    }
    let search_current = hits.rects(input.search_current.as_ref());
    let search_matches =
        hits.other_search_rects(input.search_matches, input.search_current.as_ref());
    let decorations = super::layout_decorations(&hits, &input, gutter.as_ref());
    EditorLayout {
        lines,
        line_numbers,
        viewport: Bounds::new(
            input.origin,
            size(input.viewport_width, input.viewport_height),
        ),
        origin: input.origin,
        text_origin,
        line_height: metrics.line_height,
        scroll_top,
        scroll_left,
        cursor,
        selection,
        selection_color: input.selection_color,
        search_matches,
        search_match_color: input.search_match_color,
        search_current,
        search_current_color: input.search_current_color,
        occurrences: decorations.occurrences,
        occurrence_color: input.occurrence_color,
        diagnostic_underlines: decorations.underlines,
        diagnostic_marks: decorations.marks,
        cell_width: cell_w,
        gutter,
        gutter_color: input.gutter_color,
        scrollbars,
        scrollbar_color: input.scrollbar_color,
        wrap_rows: None,
        continuation_rows: Vec::new(),
    }
}
