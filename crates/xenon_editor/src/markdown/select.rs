//! Preview text selection: carets, ranges, word/line bounds, copy join.
//! Byte offsets (UTF-8), matching GPUI `TextLayout` indices.

use std::ops::Range;

use gpui::SharedString;

/// Caret into the document-order list of selectable preview blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    pub block: usize,
    /// Byte offset within that block's plain text.
    pub offset: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SelectMode {
    #[default]
    Character,
    Word(Range<usize>),
    Line(Range<usize>),
    All,
}

#[derive(Clone, Debug, Default)]
pub struct PreviewSel {
    pub anchor: Caret,
    pub head: Caret,
    pub pending: bool,
    pub mode: SelectMode,
}

impl PreviewSel {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Normalized half-open range in document order (anchor/head either way).
    pub(crate) fn ordered(&self) -> (Caret, Caret) {
        if caret_le(self.anchor, self.head) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Local byte range within `block_ix`, if any selection covers that block.
    pub(crate) fn local_range(&self, block_ix: usize, block_len: usize) -> Option<Range<usize>> {
        if self.is_empty() {
            return None;
        }
        let (start, end) = self.ordered();
        if block_ix < start.block || block_ix > end.block {
            return None;
        }
        let from = if block_ix == start.block {
            start.offset.min(block_len)
        } else {
            0
        };
        let to = if block_ix == end.block {
            end.offset.min(block_len)
        } else {
            block_len
        };
        if from >= to { None } else { Some(from..to) }
    }

    pub(crate) fn set_caret(&mut self, caret: Caret) {
        self.anchor = caret;
        self.head = caret;
        self.pending = true;
        self.mode = SelectMode::Character;
    }

    pub(crate) fn extend_to(&mut self, caret: Caret) {
        self.head = caret;
        self.pending = true;
    }

    pub(crate) fn set_range(
        &mut self,
        anchor: Caret,
        head: Caret,
        mode: SelectMode,
        pending: bool,
    ) {
        self.anchor = anchor;
        self.head = head;
        self.mode = mode;
        self.pending = pending;
    }

    pub(crate) fn set_head_drag(&mut self, caret: Caret, text: &str) {
        match &self.mode {
            SelectMode::Word(range) => {
                let (start, end) = self.ordered();
                if caret.block == start.block && caret.block == end.block {
                    let mut a = range.clone();
                    if caret.offset < range.start {
                        a.start = caret.offset;
                    } else if caret.offset > range.end {
                        a.end = caret.offset;
                    }
                    self.anchor = Caret {
                        block: caret.block,
                        offset: a.start,
                    };
                    self.head = Caret {
                        block: caret.block,
                        offset: a.end,
                    };
                } else {
                    self.head = caret;
                }
            }
            SelectMode::Line(range) => {
                if caret.block == self.anchor.block {
                    let line = line_range_at(text, caret.offset);
                    let mut start = range.start.min(line.start);
                    let mut end = range.end.max(line.end);
                    if caret.offset < range.start {
                        start = line.start;
                        end = range.end;
                    } else if caret.offset > range.end {
                        start = range.start;
                        end = line.end;
                    }
                    self.anchor = Caret {
                        block: caret.block,
                        offset: start,
                    };
                    self.head = Caret {
                        block: caret.block,
                        offset: end,
                    };
                } else {
                    self.head = caret;
                }
            }
            SelectMode::All => {}
            SelectMode::Character => {
                self.head = caret;
            }
        }
    }
}

fn caret_le(a: Caret, b: Caret) -> bool {
    a.block < b.block || (a.block == b.block && a.offset <= b.offset)
}

/// Source markdown for the selection.
///
/// Block-granular: copies the contiguous source slice from the first selected
/// block through the last so markup (`**`, fences, list markers) stays valid.
/// Intra-block plain offsets only affect the highlight, not the clipboard.
pub(crate) fn selected_markdown(
    source: &str,
    source_ranges: &[Range<usize>],
    sel: &PreviewSel,
) -> String {
    if sel.is_empty() || source_ranges.is_empty() || source.is_empty() {
        return String::new();
    }
    if matches!(sel.mode, SelectMode::All) {
        return source.to_string();
    }
    let (start, end) = sel.ordered();
    if start.block >= source_ranges.len() {
        return String::new();
    }
    let end_block = end.block.min(source_ranges.len() - 1);
    let from = source_ranges[start.block].start.min(source.len());
    let to = source_ranges[end_block].end.min(source.len());
    if from >= to {
        return String::new();
    }
    let from = floor_char_boundary(source, from);
    let to = floor_char_boundary(source, to);
    if from >= to {
        String::new()
    } else {
        source[from..to].to_string()
    }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub(crate) fn select_all_carets(blocks: &[SharedString]) -> Option<(Caret, Caret)> {
    if blocks.is_empty() {
        return None;
    }
    let last = blocks.len() - 1;
    Some((
        Caret {
            block: 0,
            offset: 0,
        },
        Caret {
            block: last,
            offset: blocks[last].len(),
        },
    ))
}

pub(crate) fn clamp_caret(blocks: &[SharedString], caret: Caret) -> Caret {
    if blocks.is_empty() {
        return Caret::default();
    }
    let block = caret.block.min(blocks.len() - 1);
    Caret {
        block,
        offset: clamp_offset(blocks[block].as_ref(), caret.offset),
    }
}

fn clamp_offset(text: &str, offset: usize) -> usize {
    let mut o = offset.min(text.len());
    while o > 0 && !text.is_char_boundary(o) {
        o -= 1;
    }
    o
}

/// Word byte range containing `offset` (or adjacent if on a boundary).
pub(crate) fn word_range_at(text: &str, offset: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let offset = clamp_offset(text, offset.min(text.len().saturating_sub(1)));
    let ch = text[offset..].chars().next().unwrap_or(' ');
    let class = char_class(ch);
    let mut start = offset;
    while start > 0 {
        let prev = prev_boundary(text, start);
        let c = text[prev..start].chars().next().unwrap_or(' ');
        if char_class(c) != class {
            break;
        }
        start = prev;
    }
    let mut end = offset + ch.len_utf8();
    while end < text.len() {
        let c = text[end..].chars().next().unwrap_or(' ');
        if char_class(c) != class {
            break;
        }
        end += c.len_utf8();
    }
    start..end
}

/// Line content range containing `offset`, excluding a trailing `\n`.
pub(crate) fn line_range_at(text: &str, offset: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let offset = clamp_offset(text, offset.min(text.len()));
    let start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(text.len());
    start..end
}

fn prev_boundary(text: &str, offset: usize) -> usize {
    let mut i = offset - 1;
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn char_class(ch: char) -> CharClass {
    if ch.is_alphanumeric() || ch == '_' {
        CharClass::Word
    } else if ch.is_whitespace() {
        CharClass::Space
    } else {
        CharClass::Other
    }
}

/// Merge a selection background into existing non-overlapping highlight ranges.
pub(crate) fn highlights_with_selection(
    base: Vec<(Range<usize>, gpui::HighlightStyle)>,
    sel: Option<Range<usize>>,
    color: gpui::Hsla,
    text_len: usize,
) -> Vec<(Range<usize>, gpui::HighlightStyle)> {
    let Some(sel) = sel.filter(|r| r.start < r.end) else {
        return base;
    };
    let sel = sel.start.min(text_len)..sel.end.min(text_len);
    if sel.start >= sel.end {
        return base;
    }

    let mut cuts = vec![0, text_len];
    for (r, _) in &base {
        cuts.push(r.start.min(text_len));
        cuts.push(r.end.min(text_len));
    }
    cuts.push(sel.start);
    cuts.push(sel.end);
    cuts.sort_unstable();
    cuts.dedup();

    let style_at = |i: usize| -> gpui::HighlightStyle {
        let mut style = base
            .iter()
            .find(|(r, _)| i >= r.start && i < r.end)
            .map(|(_, s)| *s)
            .unwrap_or_default();
        if i >= sel.start && i < sel.end {
            style.background_color = Some(color);
        }
        style
    };

    let mut result: Vec<(Range<usize>, gpui::HighlightStyle)> = Vec::new();
    for window in cuts.windows(2) {
        let (a, b) = (window[0], window[1]);
        if a >= b {
            continue;
        }
        let style = style_at(a);
        if style == gpui::HighlightStyle::default() {
            continue;
        }
        if let Some(last) = result.last_mut()
            && last.1 == style
            && last.0.end == a
        {
            last.0.end = b;
            continue;
        }
        result.push((a..b, style));
    }
    result
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
