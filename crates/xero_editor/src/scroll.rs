//! Scroll offsets that keep the cursor inside the editor viewport.

use gpui::{Pixels, px};

/// Vertical scroll so `cursor_row` stays fully inside the viewport.
pub fn keep_row_visible(
    scroll_top: Pixels,
    cursor_row: usize,
    line_height: Pixels,
    viewport_height: Pixels,
) -> Pixels {
    if line_height <= px(0.) || viewport_height <= px(0.) {
        return scroll_top.max(px(0.));
    }
    let cursor_top = line_height * (cursor_row as f32);
    let cursor_bottom = cursor_top + line_height;
    let bottom = scroll_top + viewport_height;
    if cursor_top < scroll_top {
        cursor_top
    } else if cursor_bottom > bottom {
        (cursor_bottom - viewport_height).max(px(0.))
    } else {
        scroll_top
    }
}

/// Horizontal scroll so the cursor column stays inside the text area.
pub fn keep_col_visible(
    scroll_left: Pixels,
    cursor_col: usize,
    cell_width: Pixels,
    text_width: Pixels,
) -> Pixels {
    if cell_width <= px(0.) || text_width <= px(0.) {
        return scroll_left.max(px(0.));
    }
    let cursor_left = cell_width * (cursor_col as f32);
    let cursor_right = cursor_left + cell_width;
    let right = scroll_left + text_width;
    if cursor_left < scroll_left {
        cursor_left
    } else if cursor_right > right {
        (cursor_right - text_width).max(px(0.))
    } else {
        scroll_left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_past_bottom_scrolls() {
        // Viewport shows rows 0..9 (10 lines of height 10 = 100).
        let next = keep_row_visible(px(0.), 10, px(10.), px(100.));
        assert_eq!(next, px(10.));
    }

    #[test]
    fn up_past_top_scrolls() {
        let next = keep_row_visible(px(50.), 3, px(10.), px(100.));
        assert_eq!(next, px(30.));
    }

    #[test]
    fn already_visible_unchanged() {
        let next = keep_row_visible(px(20.), 5, px(10.), px(100.));
        assert_eq!(next, px(20.));
    }

    #[test]
    fn col_past_right_scrolls() {
        let next = keep_col_visible(px(0.), 20, px(10.), px(100.));
        assert_eq!(next, px(110.));
    }
}
