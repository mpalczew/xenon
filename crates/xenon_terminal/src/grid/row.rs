//! One terminal row as grid-anchored segments. Adapted from zed's
//! `terminal_element.rs` batched text runs, GPL-3.0-or-later; see ATTRIBUTION.md.
//!
//! Each segment is shaped with a forced per-glyph advance of one cell and
//! painted at its starting column, so a fallback glyph (✔, ❯, emoji) whose
//! natural advance differs from the cell width cannot drift later text away
//! from the cursor, selection, and background quads. A column gap (the spacer
//! after a wide character) starts a new segment.

use gpui::{Bounds, Font, Hsla, Pixels, Point, ShapedLine, SharedString, Size, TextRun, Window};
use theme::Theme;

use super::{GridLine, Viewport};
use crate::color::convert_color;

/// Consecutive cells starting at `column`.
struct Segment {
    column: i32,
    text: String,
    runs: Vec<TextRun>,
}

pub(super) struct Row {
    pub(super) line: i32,
    segments: Vec<Segment>,
    backgrounds: Vec<(i32, Hsla)>,
}

impl Row {
    pub(super) fn new(line: i32) -> Self {
        Row {
            line,
            segments: Vec::new(),
            backgrounds: Vec::new(),
        }
    }

    pub(super) fn push(&mut self, indexed: &terminal::IndexedCell, theme: &Theme, font: &Font) {
        let cell = &indexed.cell;
        if cell.is_wide_char_spacer() {
            return;
        }
        let (mut fg, mut bg) = (cell.foreground(), cell.background());
        if cell.is_inverse() {
            std::mem::swap(&mut fg, &mut bg);
        }
        let column = indexed.point.column as i32;
        if !terminal::is_default_background_color(bg) {
            self.backgrounds.push((column, convert_color(&bg, theme)));
        }
        let ch = cell.character();
        let segment = self.segment_at(column);
        segment.text.push(ch);
        segment.runs.push(TextRun {
            len: ch.len_utf8(),
            color: convert_color(&fg, theme),
            background_color: None,
            font: font.clone(),
            underline: None,
            strikethrough: None,
        });
    }

    /// The segment that `column` continues, or a new one after a gap.
    fn segment_at(&mut self, column: i32) -> &mut Segment {
        let continues = self
            .segments
            .last()
            .is_some_and(|s| s.column + s.text.chars().count() as i32 == column);
        if !continues {
            self.segments.push(Segment {
                column,
                text: String::new(),
                runs: Vec::new(),
            });
        }
        self.segments.last_mut().expect("segment was just ensured")
    }

    pub(super) fn finish(
        self,
        viewport: &Viewport,
        font_size: Pixels,
        window: &mut Window,
    ) -> GridLine {
        let y = viewport.origin.y + viewport.line_height * ((self.line + viewport.offset) as f32);
        let x_of = |column: i32| viewport.origin.x + viewport.cell_w * (column as f32);
        let segments = self
            .segments
            .into_iter()
            .map(|segment| {
                (
                    Point::new(x_of(segment.column), y),
                    shape(segment, viewport, font_size, window),
                )
            })
            .collect();
        let backgrounds = self
            .backgrounds
            .into_iter()
            .map(|(column, color)| {
                let size = Size {
                    width: viewport.cell_w,
                    height: viewport.line_height,
                };
                (Bounds::new(Point::new(x_of(column), y), size), color)
            })
            .collect();
        GridLine {
            segments,
            backgrounds,
        }
    }
}

fn shape(
    segment: Segment,
    viewport: &Viewport,
    font_size: Pixels,
    window: &mut Window,
) -> ShapedLine {
    window.text_system().shape_line(
        SharedString::from(segment.text),
        font_size,
        &segment.runs,
        Some(viewport.cell_w),
    )
}
