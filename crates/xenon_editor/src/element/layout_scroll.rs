use gpui::{Pixels, px};

use super::{LayoutInput, TextMetrics};

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
    if input.follow_cursor {
        let (width, height) = crate::scroll::follow_viewport(
            sizes.text_width,
            input.viewport_height,
            sizes.content_width,
            sizes.content_height,
            crate::scroll::scrollbar_reserve(),
        );
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
