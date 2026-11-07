//! Visual-block geometry: rectangle of (row, col) corners → per-line char ranges.

use std::ops::Range;

use ropey::Rope;

use crate::selection;

/// Inclusive block corners as zero-based (row, col).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockCorners {
    pub min_row: usize,
    pub max_row: usize,
    pub min_col: usize,
    pub max_col: usize,
}

impl BlockCorners {
    pub fn from_corners(a: (usize, usize), b: (usize, usize)) -> Self {
        Self {
            min_row: a.0.min(b.0),
            max_row: a.0.max(b.0),
            min_col: a.1.min(b.1),
            max_col: a.1.max(b.1),
        }
    }
}

/// Content length of a line (excludes trailing newline).
pub fn line_content_len(rope: &Rope, row: usize) -> usize {
    if row >= rope.len_lines() {
        return 0;
    }
    let line = rope.line(row);
    let n = line.len_chars();
    if n > 0 && line.char(n - 1) == '\n' {
        n - 1
    } else {
        n
    }
}

/// Char offset of `(row, col)`, clamped to the line's content (not past EOL).
pub fn offset_at(rope: &Rope, row: usize, col: usize) -> usize {
    let last = rope.len_lines().saturating_sub(1);
    let row = row.min(last);
    let start = rope.line_to_char(row);
    let len = line_content_len(rope, row);
    start + col.min(len)
}

/// Half-open char ranges for each line in the block (empty lines skipped when
/// the block is entirely past EOL). Ranges are top-to-bottom.
pub fn char_ranges(rope: &Rope, corners: BlockCorners) -> Vec<Range<usize>> {
    let last_row = rope.len_lines().saturating_sub(1);
    let mut out = Vec::new();
    for row in corners.min_row..=corners.max_row.min(last_row) {
        let start = rope.line_to_char(row);
        let len = line_content_len(rope, row);
        if corners.min_col >= len {
            continue;
        }
        let col_end = (corners.max_col + 1).min(len);
        let col_start = corners.min_col.min(len);
        if col_end > col_start {
            out.push(start + col_start..start + col_end);
        }
    }
    out
}

/// Yank/delete text for a block: one line of selected text per row, joined by `\n`.
/// Rows past EOL contribute an empty segment so paste can restore shape.
pub fn block_text(rope: &Rope, corners: BlockCorners) -> String {
    let last_row = rope.len_lines().saturating_sub(1);
    let mut parts = Vec::new();
    for row in corners.min_row..=corners.max_row.min(last_row) {
        let start = rope.line_to_char(row);
        let len = line_content_len(rope, row);
        if corners.min_col >= len {
            parts.push(String::new());
            continue;
        }
        let col_end = (corners.max_col + 1).min(len);
        let col_start = corners.min_col.min(len);
        if col_end > col_start {
            parts.push(rope.slice(start + col_start..start + col_end).to_string());
        } else {
            parts.push(String::new());
        }
    }
    parts.join("\n")
}

/// Line range covering the block (for indent / visual-line conversion).
pub fn line_span(rope: &Rope, corners: BlockCorners) -> Range<usize> {
    let a = selection::line_range_at(rope, offset_at(rope, corners.min_row, 0));
    let b = selection::line_range_at(rope, offset_at(rope, corners.max_row, 0));
    let mut end = a.end.max(b.end);
    let len = rope.len_chars();
    if end < len && rope.char(end) == '\n' {
        end += 1;
    }
    a.start.min(b.start)..end
}
