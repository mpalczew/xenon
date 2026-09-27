//! Editor canvas: shaped lines, cursor, and soft wrap.

use gpui::{
    Bounds, Font, FontFeatures, FontStyle, FontWeight, Hsla, Pixels, Point as GpuiPoint,
    ShapedLine, SharedString, Size, TextRun, Window, point, px,
};
use ropey::Rope;

mod decorations;
mod layout_scroll;
mod paint;
mod wrap;
use decorations::layout_decorations;
pub use decorations::{DiagnosticRange, PaintRect};

pub use paint::{line_height, paint};
pub use wrap::{WrapRow, buffer_at};

pub(super) const GUTTER_PAD_LEFT: f32 = 8.;
pub(super) const GUTTER_PAD_RIGHT: f32 = 8.;

/// A styled byte range `[start, end)` over the whole buffer.
pub struct ColoredSpan {
    pub start: usize,
    pub end: usize,
    pub color: Hsla,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
}

/// The shaped visible lines (each with its absolute row) plus the cursor.
pub struct EditorLayout {
    pub lines: Vec<(usize, ShapedLine)>,
    pub line_numbers: Vec<(usize, ShapedLine)>,
    pub viewport: Bounds<Pixels>,
    pub origin: GpuiPoint<Pixels>,
    pub text_origin: GpuiPoint<Pixels>,
    pub line_height: Pixels,
    pub scroll_top: Pixels,
    pub scroll_left: Pixels,
    pub cursor: Bounds<Pixels>,
    pub selection: Vec<Bounds<Pixels>>,
    pub selection_color: Hsla,
    /// Other search matches (not the current one).
    pub search_matches: Vec<Bounds<Pixels>>,
    pub search_match_color: Hsla,
    /// Current search match highlight.
    pub search_current: Vec<Bounds<Pixels>>,
    pub search_current_color: Hsla,
    pub occurrences: Vec<Bounds<Pixels>>,
    pub occurrence_color: Hsla,
    pub diagnostic_underlines: Vec<PaintRect>,
    pub diagnostic_marks: Vec<PaintRect>,
    pub cell_width: Pixels,
    pub gutter: Option<Bounds<Pixels>>,
    pub gutter_color: Hsla,
    pub scrollbars: Vec<Bounds<Pixels>>,
    pub scrollbar_color: Hsla,
    pub wrap_rows: Option<std::sync::Arc<Vec<WrapRow>>>,
    pub continuation_rows: Vec<usize>,
}

pub struct LayoutInput<'a> {
    pub rope: &'a Rope,
    pub cursor: (usize, usize),
    /// Half-open char selection ranges (char visual = one; block = many).
    pub selection_ranges: &'a [std::ops::Range<usize>],
    pub selection_color: Hsla,
    /// All find matches (char ranges); current is painted stronger.
    pub search_matches: &'a [std::ops::Range<usize>],
    pub search_current: Option<std::ops::Range<usize>>,
    pub search_match_color: Hsla,
    pub search_current_color: Hsla,
    pub occurrences: &'a [std::ops::Range<usize>],
    pub occurrence_color: Hsla,
    pub diagnostics: &'a [DiagnosticRange],
    pub default_color: Hsla,
    pub line_number_color: Hsla,
    pub gutter_color: Hsla,
    pub scrollbar_color: Hsla,
    pub spans: &'a [ColoredSpan],
    pub origin: GpuiPoint<Pixels>,
    pub viewport_width: Pixels,
    pub viewport_height: Pixels,
    pub scroll_top: Pixels,
    pub scroll_left: Pixels,
    pub show_line_numbers: bool,
    /// When true, scroll so the cursor stays inside the viewport.
    pub follow_cursor: bool,
    /// When true, center the cursor line (`zz`); wins over `follow_cursor`.
    pub center_cursor: bool,
    pub soft_wrap: bool,
}

pub struct TextMetrics<'a> {
    pub font: &'a Font,
    pub font_size: Pixels,
    pub line_height: Pixels,
}

/// The editor face for `family` (ligatures disabled for uniform cells).
pub fn editor_font(family: &str) -> Font {
    let mut font = gpui::font(family);
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

pub fn layout(
    input: LayoutInput<'_>,
    metrics: TextMetrics<'_>,
    window: &mut Window,
) -> EditorLayout {
    if input.soft_wrap {
        wrap::layout(input, metrics, window)
    } else {
        layout_scroll::layout_plain(input, metrics, window)
    }
}

struct VisibleRows {
    first: usize,
    last: usize,
    total: usize,
}

type ShapedRows = Vec<(usize, ShapedLine)>;

fn shape_visible_lines(
    input: &LayoutInput<'_>,
    metrics: &TextMetrics<'_>,
    rows: VisibleRows,
    window: &mut Window,
) -> (ShapedRows, ShapedRows) {
    let mut lines = Vec::with_capacity(rows.last.saturating_sub(rows.first));
    let mut line_numbers = Vec::with_capacity(lines.capacity());
    for row in rows.first..rows.last {
        let line_start = input.rope.line_to_byte(row);
        let text = trim_newline(input.rope.line(row).to_string());
        let runs = line_runs(
            LineSlice {
                text: &text,
                start: line_start,
            },
            input.default_color,
            input.spans,
            metrics.font,
        );
        lines.push((row, shape(text, runs, metrics.font_size, window)));
        if input.show_line_numbers {
            let digits = row_digits(rows.total);
            let number = format!("{:>digits$}", row + 1);
            line_numbers.push((
                row,
                shape(
                    number.clone(),
                    vec![run(
                        number.chars().count(),
                        input.line_number_color,
                        metrics.font,
                        None,
                        None,
                    )],
                    metrics.font_size,
                    window,
                ),
            ));
        }
    }
    (lines, line_numbers)
}

struct HitLayout<'a> {
    rope: &'a Rope,
    text_origin: GpuiPoint<Pixels>,
    origin_y: Pixels,
    cell_w: Pixels,
    line_height: Pixels,
    scroll_top: Pixels,
    first_row: usize,
    last_row: usize,
    wrap_rows: Option<&'a [WrapRow]>,
}

impl HitLayout<'_> {
    fn rects(&self, range: Option<&std::ops::Range<usize>>) -> Vec<Bounds<Pixels>> {
        selection_rects(self, range)
    }

    fn other_search_rects(
        &self,
        matches: &[std::ops::Range<usize>],
        current: Option<&std::ops::Range<usize>>,
    ) -> Vec<Bounds<Pixels>> {
        let mut out = Vec::new();
        for m in matches {
            if current == Some(m) {
                continue;
            }
            out.extend(self.rects(Some(m)));
        }
        out
    }
}

fn selection_rects(
    input: &HitLayout<'_>,
    selection: Option<&std::ops::Range<usize>>,
) -> Vec<Bounds<Pixels>> {
    if let Some(rows) = input.wrap_rows {
        return wrap::paint_ranges(input, rows, selection);
    }
    let Some(range) = selection else {
        return Vec::new();
    };
    if range.start >= range.end {
        return Vec::new();
    }
    let rope = input.rope;
    let start_row = rope.char_to_line(range.start.min(rope.len_chars().saturating_sub(1)));
    let end_idx = range.end.min(rope.len_chars());
    let end_row = if end_idx == 0 {
        0
    } else {
        rope.char_to_line(end_idx.saturating_sub(1))
    };
    let mut rects = Vec::new();
    let last_visible = input.last_row.saturating_sub(1);
    for row in start_row.max(input.first_row)..=end_row.min(last_visible) {
        let line_start = rope.line_to_char(row);
        let line_end = line_start + line_len_chars(rope, row);
        let sel_start = range.start.max(line_start);
        let sel_end = range.end.min(line_end);
        let y = input.origin_y + input.line_height * (row as f32) - input.scroll_top;
        if sel_end <= sel_start {
            if range.start <= line_start && range.end > line_start {
                rects.push(Bounds::new(
                    point(input.text_origin.x, y),
                    Size {
                        width: input.cell_w * 0.5,
                        height: input.line_height,
                    },
                ));
            }
            continue;
        }
        let col_start = sel_start - line_start;
        let col_end = sel_end - line_start;
        let x = input.text_origin.x + input.cell_w * (col_start as f32);
        let width = input.cell_w * ((col_end - col_start) as f32).max(0.5);
        rects.push(Bounds::new(
            point(x, y),
            Size {
                width,
                height: input.line_height,
            },
        ));
    }
    rects
}

fn gutter_width(rows: usize, cell_w: Pixels, show: bool) -> Pixels {
    if show {
        px(GUTTER_PAD_LEFT) + cell_w * (row_digits(rows) as f32) + px(GUTTER_PAD_RIGHT)
    } else {
        px(0.)
    }
}

fn row_digits(rows: usize) -> usize {
    rows.max(1).to_string().len()
}

fn content_width(rope: &Rope, cell_w: Pixels) -> Pixels {
    let cols = (0..rope.len_lines())
        .map(|row| line_len_chars(rope, row))
        .max()
        .unwrap_or(0);
    cell_w * (cols as f32)
}

fn line_len_chars(rope: &Rope, row: usize) -> usize {
    let line = rope.line(row);
    let len = line.len_chars();
    if len > 0 && line.char(len - 1) == '\n' {
        len - 1
    } else {
        len
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

/// Split one line into text runs: span styles where they cover the line,
/// `default_color` in the gaps. Spans are sorted, non-overlapping (source order).
/// A line's text together with its byte offset in the rope.
struct LineSlice<'a> {
    text: &'a str,
    start: usize,
}

fn line_runs(
    line: LineSlice,
    default_color: Hsla,
    spans: &[ColoredSpan],
    font: &Font,
) -> Vec<TextRun> {
    let line_start = line.start;
    let line_end = line_start + line.text.len();
    let mut runs = Vec::new();
    let mut pos = line_start;
    for span in spans {
        if span.end <= line_start || span.start >= line_end {
            continue;
        }
        let start = span.start.max(line_start);
        let end = span.end.min(line_end);
        if start > pos {
            runs.push(run(start - pos, default_color, font, None, None));
        }
        if end > start {
            runs.push(run(
                end - start,
                span.color,
                font,
                span.font_weight,
                span.font_style,
            ));
            pos = end;
        }
    }
    if pos < line_end {
        runs.push(run(line_end - pos, default_color, font, None, None));
    }
    runs
}

fn run(
    len: usize,
    color: Hsla,
    font: &Font,
    weight: Option<FontWeight>,
    style: Option<FontStyle>,
) -> TextRun {
    let mut font = font.clone();
    if let Some(w) = weight {
        font.weight = w;
    }
    if let Some(s) = style {
        font.style = s;
    }
    TextRun {
        len,
        color,
        background_color: None,
        font,
        underline: None,
        strikethrough: None,
    }
}

fn shape(text: String, runs: Vec<TextRun>, font_size: Pixels, window: &mut Window) -> ShapedLine {
    window
        .text_system()
        .shape_line(SharedString::from(text), font_size, &runs, None)
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use gpui::hsla;

    #[test]
    fn run_applies_font_weight_and_style() {
        let base = editor_font("Menlo");
        let bold = run(
            4,
            hsla(0., 0., 1., 1.),
            &base,
            Some(FontWeight::BOLD),
            Some(FontStyle::Italic),
        );
        assert_eq!(bold.font.weight, FontWeight::BOLD);
        assert_eq!(bold.font.style, FontStyle::Italic);
        assert_eq!(bold.font.family, base.family);
    }
}
