//! Soft wrap: one buffer line paints as several visual rows. The file is unchanged.

use std::sync::Arc;

use gpui::{Bounds, Font, Pixels, Size, point, px, size};
use ropey::Rope;

use super::{
    EditorLayout, HitLayout, LayoutInput, LineSlice, TextMetrics, cell_width, gutter_width,
    layout_decorations, line_len_chars, line_runs, run, shape,
};

/// One visual row of a buffer line. `start`/`end` are char offsets, excluding the newline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrapRow {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub continuation: bool,
}

pub(super) fn layout(
    input: LayoutInput<'_>,
    metrics: TextMetrics<'_>,
    window: &mut gpui::Window,
) -> EditorLayout {
    let cell_w = cell_width(window, metrics.font, metrics.font_size);
    let gutter_width = gutter_width(input.rope.len_lines(), cell_w, input.show_line_numbers);
    let text_width = (input.viewport_width - gutter_width).max(cell_w);
    let cols = cols_for(text_width, cell_w);
    let mut rows = rows_for(input.rope, cols);
    let mut content_height = metrics.line_height * (rows.len() as f32);
    if content_height > input.viewport_height {
        let narrower = (text_width - crate::scroll::scrollbar_reserve()).max(cell_w);
        let fitted = cols_for(narrower, cell_w);
        if fitted != cols {
            rows = rows_for(input.rope, fitted);
            content_height = metrics.line_height * (rows.len() as f32);
        }
    }
    let rows = Arc::new(rows);
    let (cursor_line, cursor_col) = input.cursor;
    let (visual, local_col) = locate(&rows, cursor_line, cursor_col);
    let (_view_w, view_h) = crate::scroll::follow_viewport(
        text_width,
        input.viewport_height,
        px(0.),
        content_height,
        crate::scroll::scrollbar_reserve(),
    );
    let mut scroll_top = input.scroll_top;
    if input.center_cursor {
        scroll_top = crate::scroll::center_row(visual, metrics.line_height, view_h);
    } else if input.follow_cursor {
        scroll_top =
            crate::scroll::keep_row_visible(scroll_top, visual, metrics.line_height, view_h);
    }
    let scroll_top = scroll_top
        .min((content_height - input.viewport_height).max(px(0.)))
        .max(px(0.));
    let scroll_left = px(0.);
    let text_origin = point(input.origin.x + gutter_width, input.origin.y);
    let gutter = input
        .show_line_numbers
        .then(|| Bounds::new(input.origin, size(gutter_width, input.viewport_height)));
    let scrollbars = crate::scroll::scrollbars(crate::scroll::ScrollbarInput {
        origin: input.origin,
        viewport_width: input.viewport_width,
        viewport_height: input.viewport_height,
        gutter_width,
        text_width,
        content_width: px(0.),
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
    let last = (first + visible).min(rows.len());
    let shaped = shape_visible(
        ShapeIn {
            input: &input,
            metrics: &metrics,
            rows: &rows,
            first,
            last,
            total,
        },
        window,
    );
    let cursor = Bounds::new(
        point(
            text_origin.x + cell_w * (local_col as f32),
            input.origin.y + metrics.line_height * (visual as f32) - scroll_top,
        ),
        size(cell_w, metrics.line_height),
    );
    compose(
        input,
        &metrics,
        Frame {
            cell_w,
            text_origin,
            gutter,
            scrollbars,
            scroll_top,
            scroll_left,
            cursor,
            first,
            last,
        },
        shaped,
        rows,
    )
}

struct Frame {
    cell_w: Pixels,
    text_origin: gpui::Point<Pixels>,
    gutter: Option<Bounds<Pixels>>,
    scrollbars: Vec<Bounds<Pixels>>,
    scroll_top: Pixels,
    scroll_left: Pixels,
    cursor: Bounds<Pixels>,
    first: usize,
    last: usize,
}

fn compose(
    input: LayoutInput<'_>,
    metrics: &TextMetrics<'_>,
    frame: Frame,
    shaped: Shaped,
    rows: Arc<Vec<WrapRow>>,
) -> EditorLayout {
    let hits = HitLayout {
        rope: input.rope,
        text_origin: frame.text_origin,
        origin_y: input.origin.y,
        cell_w: frame.cell_w,
        line_height: metrics.line_height,
        scroll_top: frame.scroll_top,
        first_row: frame.first,
        last_row: frame.last,
        wrap_rows: Some(rows.as_ref()),
    };
    let mut selection = Vec::new();
    for range in input.selection_ranges {
        selection.extend(hits.rects(Some(range)));
    }
    let search_current = hits.rects(input.search_current.as_ref());
    let search_matches =
        hits.other_search_rects(input.search_matches, input.search_current.as_ref());
    let decorations = layout_decorations(&hits, &input, frame.gutter.as_ref());
    EditorLayout {
        lines: shaped.lines,
        line_numbers: shaped.numbers,
        viewport: Bounds::new(
            input.origin,
            size(input.viewport_width, input.viewport_height),
        ),
        origin: input.origin,
        text_origin: frame.text_origin,
        line_height: metrics.line_height,
        scroll_top: frame.scroll_top,
        scroll_left: frame.scroll_left,
        cursor: frame.cursor,
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
        cell_width: frame.cell_w,
        gutter: frame.gutter,
        gutter_color: input.gutter_color,
        scrollbars: frame.scrollbars,
        scrollbar_color: input.scrollbar_color,
        wrap_rows: Some(rows),
        continuation_rows: shaped.continuations,
    }
}

struct ShapeIn<'a> {
    input: &'a LayoutInput<'a>,
    metrics: &'a TextMetrics<'a>,
    rows: &'a [WrapRow],
    first: usize,
    last: usize,
    total: usize,
}

struct Shaped {
    lines: Vec<(usize, gpui::ShapedLine)>,
    numbers: Vec<(usize, gpui::ShapedLine)>,
    continuations: Vec<usize>,
}

fn cols_for(text_width: Pixels, cell_w: Pixels) -> usize {
    if f32::from(cell_w) <= 0. {
        return 1;
    }
    (f32::from(text_width) / f32::from(cell_w)).floor().max(1.) as usize
}

pub(super) fn rows_for(rope: &Rope, cols: usize) -> Vec<WrapRow> {
    let mut out = Vec::new();
    for line in 0..rope.len_lines() {
        let start = rope.line_to_char(line);
        let len = line_len_chars(rope, line);
        let text = rope.slice(start..start + len).to_string();
        push_line(line, &text, cols, &mut out);
    }
    if out.is_empty() {
        out.push(WrapRow {
            line: 0,
            start: 0,
            end: 0,
            continuation: false,
        });
    }
    out
}

fn push_line(line: usize, text: &str, cols: usize, out: &mut Vec<WrapRow>) {
    let chars: Vec<char> = text.chars().collect();
    let cols = cols.max(1);
    if chars.is_empty() {
        out.push(WrapRow {
            line,
            start: 0,
            end: 0,
            continuation: false,
        });
        return;
    }
    let mut start = 0;
    let mut continuation = false;
    while start < chars.len() {
        if chars.len() - start <= cols {
            out.push(WrapRow {
                line,
                start,
                end: chars.len(),
                continuation,
            });
            return;
        }
        let mut end = start + cols;
        let mut index = end;
        while index > start {
            if chars[index - 1].is_whitespace() {
                end = index;
                break;
            }
            index -= 1;
        }
        if end == start {
            end = (start + cols).min(chars.len());
        }
        out.push(WrapRow {
            line,
            start,
            end,
            continuation,
        });
        start = end;
        continuation = true;
    }
}

pub(super) fn locate(rows: &[WrapRow], line: usize, col: usize) -> (usize, usize) {
    let mut fallback = (0usize, 0usize);
    for (index, row) in rows.iter().enumerate() {
        if row.line != line {
            continue;
        }
        let span = row.end.saturating_sub(row.start);
        let last = rows.get(index + 1).is_none_or(|next| next.line != row.line);
        fallback = (index, span);
        if col >= row.start && (col < row.end || last) {
            return (index, (col - row.start).min(span));
        }
    }
    fallback
}

/// Map a click on visual row `visual` to a buffer `(line, col)`.
pub fn buffer_at(rows: &[WrapRow], visual: usize, local_col: usize) -> (usize, usize) {
    if rows.is_empty() {
        return (0, 0);
    }
    let index = visual.min(rows.len() - 1);
    let row = &rows[index];
    let last = rows.get(index + 1).is_none_or(|next| next.line != row.line);
    let span = row.end.saturating_sub(row.start);
    let local = local_col.min(span);
    if last || span == 0 {
        (row.line, row.start + local)
    } else if local >= span {
        (row.line, row.end - 1)
    } else {
        (row.line, row.start + local)
    }
}

pub(super) fn paint_ranges(
    hits: &HitLayout<'_>,
    rows: &[WrapRow],
    selection: Option<&std::ops::Range<usize>>,
) -> Vec<Bounds<Pixels>> {
    let Some(range) = selection else {
        return Vec::new();
    };
    if range.start >= range.end {
        return Vec::new();
    }
    let rope = hits.rope;
    let last_visible = hits.last_row.saturating_sub(1);
    let mut rects = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index < hits.first_row || index > last_visible {
            continue;
        }
        let line_start = rope.line_to_char(row.line);
        let seg_start = line_start + row.start;
        let seg_end = line_start + row.end;
        let y = hits.origin_y + hits.line_height * (index as f32) - hits.scroll_top;
        if row.start == row.end {
            if range.start <= seg_start && range.end > seg_start {
                rects.push(Bounds::new(
                    point(hits.text_origin.x, y),
                    Size {
                        width: hits.cell_w * 0.5,
                        height: hits.line_height,
                    },
                ));
            }
            continue;
        }
        let sel_start = range.start.max(seg_start);
        let sel_end = range.end.min(seg_end);
        if sel_end <= sel_start {
            continue;
        }
        let col_start = sel_start - seg_start;
        let col_end = sel_end - seg_start;
        rects.push(Bounds::new(
            point(hits.text_origin.x + hits.cell_w * (col_start as f32), y),
            Size {
                width: hits.cell_w * ((col_end - col_start) as f32).max(0.5),
                height: hits.line_height,
            },
        ));
    }
    rects
}

fn shape_visible(src: ShapeIn<'_>, window: &mut gpui::Window) -> Shaped {
    let ShapeIn {
        input,
        metrics,
        rows,
        first,
        last,
        total,
    } = src;
    let mut lines = Vec::with_capacity(last.saturating_sub(first));
    let mut numbers = Vec::with_capacity(lines.capacity());
    let mut continuations = Vec::new();
    for (index, row) in rows.iter().enumerate().skip(first).take(last - first) {
        let line_char = input.rope.line_to_char(row.line);
        let start_char = line_char + row.start;
        let end_char = line_char + row.end;
        let text = input.rope.slice(start_char..end_char).to_string();
        let byte_start = input.rope.char_to_byte(start_char);
        let runs = line_runs(
            LineSlice {
                text: &text,
                start: byte_start,
            },
            input.default_color,
            input.spans,
            metrics.font,
        );
        lines.push((index, shape(text, runs, metrics.font_size, window)));
        if !input.show_line_numbers {
            continue;
        }
        if row.continuation {
            continuations.push(index);
            numbers.push((index, continuation_mark(input.line_number_color, window)));
        } else {
            let digits = super::row_digits(total);
            let number = format!("{:>digits$}", row.line + 1);
            numbers.push((
                index,
                shape(
                    number.clone(),
                    vec![run(
                        number.len(),
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
    Shaped {
        lines,
        numbers,
        continuations,
    }
}

/// Lucide `corner-down-right` (U+E0A2 in the app icon font).
fn continuation_mark(color: gpui::Hsla, window: &mut gpui::Window) -> gpui::ShapedLine {
    let text = "\u{e0a2}";
    let font: Font = gpui::font("lucide");
    shape(
        text.to_string(),
        vec![run(text.len(), color, &font, None, None)],
        px(11.),
        window,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str, cols: usize) -> Vec<WrapRow> {
        rows_for(&Rope::from(text), cols)
    }

    #[test]
    fn short_line_stays_one_row() {
        let wrapped = rows("hello", 20);
        assert_eq!(
            wrapped,
            vec![WrapRow {
                line: 0,
                start: 0,
                end: 5,
                continuation: false,
            }]
        );
    }

    #[test]
    fn breaks_on_the_last_space_that_fits() {
        let wrapped = rows("The preview already wraps\n", 16);
        assert_eq!(wrapped[0].end, 12);
        assert!(!wrapped[0].continuation);
        assert!(wrapped[1].continuation);
        assert_eq!(wrapped[1].start, 12);
        assert_eq!(&"The preview "[..wrapped[0].end], "The preview ");
    }

    #[test]
    fn hard_breaks_a_word_longer_than_the_pane() {
        let wrapped = rows("abcdefghijKLM", 10);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].end, 10);
        assert_eq!(wrapped[1].start, 10);
        assert!(wrapped[1].continuation);
    }

    #[test]
    fn click_on_a_continuation_stays_on_that_row() {
        let wrapped = rows("abcdefghijKLM\n", 10);
        assert_eq!(buffer_at(&wrapped, 1, 2), (0, 12));
        assert_eq!(buffer_at(&wrapped, 0, 40), (0, 9));
        assert_eq!(locate(&wrapped, 0, 10), (1, 0));
        assert_eq!(locate(&wrapped, 0, 12), (1, 2));
    }
}
