//! Paint the laid-out editor canvas.

use gpui::{ContentMask, Hsla, Pixels, TextAlign, Window, fill, point, px};

use super::{EditorLayout, GUTTER_PAD_LEFT};

/// Paint search highlights, selection, cursor, then the visible shaped lines.
pub fn paint(layout: &EditorLayout, cursor_color: Hsla, window: &mut Window, cx: &mut gpui::App) {
    window.with_content_mask(
        Some(ContentMask {
            bounds: layout.viewport,
        }),
        |window| {
            for rect in &layout.search_matches {
                window.paint_quad(fill(*rect, layout.search_match_color));
            }
            for rect in &layout.search_current {
                window.paint_quad(fill(*rect, layout.search_current_color));
            }
            for rect in &layout.selection {
                window.paint_quad(fill(*rect, layout.selection_color));
            }
            window.paint_quad(fill(layout.cursor, cursor_color));
            for (row, line) in &layout.lines {
                let y = layout.origin.y + layout.line_height * (*row as f32) - layout.scroll_top;
                let _ = line.paint(
                    point(layout.text_origin.x, y),
                    layout.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            if let Some(gutter) = layout.gutter {
                window.paint_quad(fill(gutter, layout.gutter_color));
            }
            for (row, line) in &layout.line_numbers {
                let y = layout.origin.y + layout.line_height * (*row as f32) - layout.scroll_top;
                let _ = line.paint(
                    point(layout.origin.x + px(GUTTER_PAD_LEFT), y),
                    layout.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for scrollbar in &layout.scrollbars {
                window.paint_quad(fill(*scrollbar, layout.scrollbar_color));
            }
        },
    );
}

/// Line height in pixels for a font size and multiplier.
pub fn line_height(font_size: Pixels, multiplier: f32) -> Pixels {
    px(f32::from(font_size) * multiplier)
}
