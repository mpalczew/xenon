//! Editor rendering: shape each line of the buffer and place the cursor. Plain
//! text for now; syntax highlighting will add per-span colors here later.

use gpui::{
    Bounds, Font, FontFeatures, Hsla, Pixels, Point as GpuiPoint, ShapedLine, SharedString, Size,
    TextAlign, TextRun, Window, fill, point, px,
};
use ropey::Rope;

/// A shaped buffer with the cursor rectangle to overlay.
pub struct EditorLayout {
    pub lines: Vec<ShapedLine>,
    pub origin: GpuiPoint<Pixels>,
    pub line_height: Pixels,
    pub cursor: Bounds<Pixels>,
}

/// The monospace font used for the editor (Menlo, no ligatures).
pub fn editor_font() -> Font {
    let mut font = gpui::font("Menlo");
    font.features = FontFeatures::disable_ligatures();
    font
}

fn cell_width(window: &Window, font: &Font, font_size: Pixels) -> Pixels {
    let font_id = window.text_system().resolve_font(font);
    window
        .text_system()
        .advance(font_id, font_size, 'm')
        .map(|size| size.width)
        .unwrap_or(font_size * 0.6)
}

/// Shape every line and compute the cursor rectangle from `(row, col)`.
pub fn layout(
    rope: &Rope,
    cursor: (usize, usize),
    color: Hsla,
    origin: GpuiPoint<Pixels>,
    font: &Font,
    font_size: Pixels,
    line_height: Pixels,
    window: &mut Window,
) -> EditorLayout {
    let cell_w = cell_width(window, font, font_size);
    let lines = rope
        .lines()
        .map(|line| shape(line.to_string(), color, font, font_size, window))
        .collect();

    let (row, col) = cursor;
    let cursor_origin = point(
        origin.x + cell_w * (col as f32),
        origin.y + line_height * (row as f32),
    );
    let cursor = Bounds::new(cursor_origin, Size { width: cell_w, height: line_height });
    EditorLayout { lines, origin, line_height, cursor }
}

fn shape(
    mut text: String,
    color: Hsla,
    font: &Font,
    font_size: Pixels,
    window: &mut Window,
) -> ShapedLine {
    // ropey lines carry their trailing newline; shaping must not see it.
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    let run = TextRun {
        len: text.len(),
        color,
        background_color: None,
        font: font.clone(),
        underline: None,
        strikethrough: None,
    };
    let runs = if text.is_empty() { Vec::new() } else { vec![run] };
    window
        .text_system()
        .shape_line(SharedString::from(text), font_size, &runs, None)
}

/// Paint the cursor, then the shaped lines on top.
pub fn paint(layout: &EditorLayout, cursor_color: Hsla, window: &mut Window, cx: &mut gpui::App) {
    window.paint_quad(fill(layout.cursor, cursor_color));
    for (row, line) in layout.lines.iter().enumerate() {
        let y = layout.origin.y + layout.line_height * (row as f32);
        let _ = line.paint(
            point(layout.origin.x, y),
            layout.line_height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

/// Line height in pixels for a font size and multiplier.
pub fn line_height(font_size: Pixels, multiplier: f32) -> Pixels {
    px(f32::from(font_size) * multiplier)
}
