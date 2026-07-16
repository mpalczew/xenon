//! Scroll offsets that keep the cursor inside the editor viewport,
//! plus overlay scrollbar geometry.

use gpui::{Bounds, Pixels, Point as GpuiPoint, point, px, size};

/// Overlay track thickness (thumb width/height).
pub const SCROLLBAR_SIZE: f32 = 6.;
/// Gap between viewport edge and thumb.
pub const SCROLLBAR_INSET: f32 = 2.;
const MIN_SCROLLBAR_THUMB: f32 = 24.;

/// Full overlay reservation for follow-scroll (size + inset).
pub fn scrollbar_reserve() -> Pixels {
    px(SCROLLBAR_SIZE + SCROLLBAR_INSET)
}

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

/// Shrink the text viewport by overlay scrollbar tracks so follow-scroll keeps
/// the caret clear of the bars (bars paint on top of the canvas edge).
pub fn follow_viewport(
    text_width: Pixels,
    viewport_height: Pixels,
    content_width: Pixels,
    content_height: Pixels,
    bar: Pixels,
) -> (Pixels, Pixels) {
    let mut view_w = text_width;
    let mut view_h = viewport_height;
    let v_bar = content_height > view_h;
    if v_bar {
        view_w = (view_w - bar).max(px(0.));
    }
    let h_bar = content_width > view_w;
    if h_bar {
        view_h = (view_h - bar).max(px(0.));
        // H track can force a V bar that was not needed on the full height.
        if !v_bar && content_height > view_h {
            view_w = (text_width - bar).max(px(0.));
        }
    }
    (view_w, view_h)
}

/// Inputs for painting overlay scrollbar thumbs.
pub struct ScrollbarInput {
    pub origin: GpuiPoint<Pixels>,
    pub viewport_width: Pixels,
    pub viewport_height: Pixels,
    pub gutter_width: Pixels,
    pub text_width: Pixels,
    pub content_width: Pixels,
    pub content_height: Pixels,
    pub scroll_top: Pixels,
    pub scroll_left: Pixels,
}

/// Vertical and/or horizontal overlay thumbs (empty when content fits).
pub fn scrollbars(input: ScrollbarInput) -> Vec<Bounds<Pixels>> {
    let mut bars = Vec::new();
    if input.content_height > input.viewport_height {
        bars.push(vertical_scrollbar(&input));
    }
    if input.content_width > input.text_width {
        bars.push(horizontal_scrollbar(&input));
    }
    bars
}

fn vertical_scrollbar(input: &ScrollbarInput) -> Bounds<Pixels> {
    let track = input.viewport_height - px(SCROLLBAR_INSET * 2.);
    let thumb = ((input.viewport_height / input.content_height) * track)
        .max(px(MIN_SCROLLBAR_THUMB))
        .min(track);
    let max_offset = (input.content_height - input.viewport_height).max(px(1.));
    let travel = (track - thumb).max(px(0.));
    let top = input.origin.y + px(SCROLLBAR_INSET) + travel * (input.scroll_top / max_offset);
    Bounds::new(
        point(
            input.origin.x + input.viewport_width - scrollbar_reserve(),
            top,
        ),
        size(px(SCROLLBAR_SIZE), thumb),
    )
}

fn horizontal_scrollbar(input: &ScrollbarInput) -> Bounds<Pixels> {
    let track = (input.text_width - px(SCROLLBAR_INSET * 2.)).max(px(0.));
    let thumb = ((input.text_width / input.content_width) * track)
        .max(px(MIN_SCROLLBAR_THUMB))
        .min(track);
    let max_offset = (input.content_width - input.text_width).max(px(1.));
    let travel = (track - thumb).max(px(0.));
    let left = input.origin.x
        + input.gutter_width
        + px(SCROLLBAR_INSET)
        + travel * (input.scroll_left / max_offset);
    Bounds::new(
        point(
            left,
            input.origin.y + input.viewport_height - scrollbar_reserve(),
        ),
        size(thumb, px(SCROLLBAR_SIZE)),
    )
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

    #[test]
    fn follow_viewport_reserves_vertical_bar() {
        let (w, h) = follow_viewport(px(200.), px(100.), px(50.), px(200.), px(8.));
        assert_eq!(w, px(192.));
        assert_eq!(h, px(100.));
    }

    #[test]
    fn follow_viewport_reserves_both_bars() {
        let (w, h) = follow_viewport(px(200.), px(100.), px(400.), px(200.), px(8.));
        assert_eq!(w, px(192.));
        assert_eq!(h, px(92.));
    }

    #[test]
    fn newline_at_bottom_clears_horizontal_bar() {
        // Full height 100, H bar 8 → follow height 92. Cursor row 9 (y 90..100)
        // must scroll so row bottom is at 92, not under the bar.
        let bar = px(8.);
        let (_w, view_h) = follow_viewport(px(200.), px(100.), px(400.), px(50.), bar);
        assert_eq!(view_h, px(92.));
        let next = keep_row_visible(px(0.), 9, px(10.), view_h);
        assert_eq!(next, px(8.));
    }
}
