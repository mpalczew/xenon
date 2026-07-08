//! Editor rendering: shape each line of the buffer with syntax colors and place
//! the cursor. Highlight spans arrive pre-resolved to colors (byte ranges over
//! the whole rope); gaps fall back to the default text color.

use gpui::{
    Bounds, Font, FontFeatures, Hsla, Pixels, Point as GpuiPoint, ShapedLine, SharedString, Size,
    TextAlign, TextRun, Window, fill, point, px,
};
use ropey::Rope;

/// A colored byte range `[start, end)` over the whole buffer.
pub struct ColoredSpan {
    pub start: usize,
    pub end: usize,
    pub color: Hsla,
}

/// The shaped visible lines (each with its absolute row) plus the cursor.
pub struct EditorLayout {
    pub lines: Vec<(usize, ShapedLine)>,
    pub origin: GpuiPoint<Pixels>,
    pub line_height: Pixels,
    pub scroll_top: Pixels,
    pub cursor: Bounds<Pixels>,
}

pub struct LayoutInput<'a> {
    pub rope: &'a Rope,
    pub cursor: (usize, usize),
    pub default_color: Hsla,
    pub spans: &'a [ColoredSpan],
    pub origin: GpuiPoint<Pixels>,
    pub viewport_height: Pixels,
    pub scroll_top: Pixels,
}

pub struct TextMetrics<'a> {
    pub font: &'a Font,
    pub font_size: Pixels,
    pub line_height: Pixels,
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

/// Shape every line with its highlight colors and compute the cursor rectangle.
pub fn layout(
    input: LayoutInput<'_>,
    metrics: TextMetrics<'_>,
    window: &mut Window,
) -> EditorLayout {
    let cell_w = cell_width(window, metrics.font, metrics.font_size);
    // Only shape the rows in view (plus one), offset by the scroll position.
    let total = input.rope.len_lines();
    let first = (f32::from(input.scroll_top) / f32::from(metrics.line_height))
        .floor()
        .max(0.) as usize;
    let visible =
        (f32::from(input.viewport_height) / f32::from(metrics.line_height)).ceil() as usize + 1;
    let last = (first + visible).min(total);

    let mut lines = Vec::with_capacity(last.saturating_sub(first));
    for row in first..last {
        let line_start = input.rope.line_to_byte(row);
        let text = trim_newline(input.rope.line(row).to_string());
        let runs = line_runs(
            &text,
            line_start,
            input.default_color,
            input.spans,
            metrics.font,
        );
        lines.push((row, shape(text, runs, metrics.font_size, window)));
    }

    let (row, col) = input.cursor;
    let cursor_origin = point(
        input.origin.x + cell_w * (col as f32),
        input.origin.y + metrics.line_height * (row as f32) - input.scroll_top,
    );
    let cursor = Bounds::new(
        cursor_origin,
        Size {
            width: cell_w,
            height: metrics.line_height,
        },
    );
    EditorLayout {
        lines,
        origin: input.origin,
        line_height: metrics.line_height,
        scroll_top: input.scroll_top,
        cursor,
    }
}

fn trim_newline(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    text
}

/// Split one line into text runs: span colors where they cover the line,
/// `default_color` in the gaps. Spans are sorted, non-overlapping (source order).
fn line_runs(
    text: &str,
    line_start: usize,
    default_color: Hsla,
    spans: &[ColoredSpan],
    font: &Font,
) -> Vec<TextRun> {
    let line_end = line_start + text.len();
    let mut runs = Vec::new();
    let mut pos = line_start;
    for span in spans {
        if span.end <= line_start || span.start >= line_end {
            continue;
        }
        let start = span.start.max(line_start);
        let end = span.end.min(line_end);
        if start > pos {
            runs.push(run(start - pos, default_color, font));
        }
        if end > start {
            runs.push(run(end - start, span.color, font));
            pos = end;
        }
    }
    if pos < line_end {
        runs.push(run(line_end - pos, default_color, font));
    }
    runs
}

fn run(len: usize, color: Hsla, font: &Font) -> TextRun {
    TextRun {
        len,
        color,
        background_color: None,
        font: font.clone(),
        underline: None,
        strikethrough: None,
    }
}

fn shape(text: String, runs: Vec<TextRun>, font_size: Pixels, window: &mut Window) -> ShapedLine {
    window
        .text_system()
        .shape_line(SharedString::from(text), font_size, &runs, None)
}

/// Paint the cursor, then the visible shaped lines on top.
pub fn paint(layout: &EditorLayout, cursor_color: Hsla, window: &mut Window, cx: &mut gpui::App) {
    window.paint_quad(fill(layout.cursor, cursor_color));
    for (row, line) in &layout.lines {
        let y = layout.origin.y + layout.line_height * (*row as f32) - layout.scroll_top;
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
