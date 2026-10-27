//! Model step two: leaves to blocks (inline pieces and leads).

use std::ops::Range;

use super::*;

/// Per-char style for one leaf's source region.
pub(super) struct StyleMap {
    pub(super) base: usize,
    pub(super) flags: Vec<Flags>,
    pub(super) marker: Vec<bool>,
}

impl StyleMap {
    pub(super) fn new(leaf: &Leaf, base: usize, end: usize) -> StyleMap {
        let n = end.saturating_sub(base);
        let mut map = StyleMap {
            base,
            flags: vec![0; n],
            marker: vec![false; n],
        };
        for span in &leaf.spans {
            map.apply(span);
        }
        map
    }

    pub(super) fn apply(&mut self, span: &Span) {
        let flags = span.flags;
        for pos in span.range.clone() {
            if let Some(slot) = self.flags.get_mut(pos.wrapping_sub(self.base)) {
                *slot |= if span.url { 0 } else { flags };
            }
        }
        if span.url {
            // Link text takes link style; the `](url)` tail is a dim URL.
            let marker_ranges = &span.markers;
            for pos in span.range.clone() {
                let in_marker = marker_ranges.iter().any(|m| m.contains(&pos));
                if let Some(slot) = self.flags.get_mut(pos.wrapping_sub(self.base)) {
                    *slot |= if in_marker { URL } else { LINK };
                }
            }
        }
        for marker in &span.markers {
            for pos in marker.clone() {
                if let Some(slot) = self.marker.get_mut(pos.wrapping_sub(self.base)) {
                    *slot = true;
                }
            }
        }
    }

    pub(super) fn at(&self, pos: usize) -> (Flags, bool) {
        let i = pos.wrapping_sub(self.base);
        (
            self.flags.get(i).copied().unwrap_or(0),
            self.marker.get(i).copied().unwrap_or(false),
        )
    }
}

/// Group `range` into pieces by (marker, flags).
pub(super) fn styled_pieces(map: &StyleMap, range: Range<usize>, out: &mut Vec<Piece>) {
    let mut start = range.start;
    while start < range.end {
        let key = map.at(start);
        let mut end = start + 1;
        while end < range.end && map.at(end) == key {
            end += 1;
        }
        out.push(Piece {
            src: start..end,
            kind: if key.1 {
                PieceKind::Marker
            } else {
                PieceKind::Text
            },
            flags: key.0,
        });
        start = end;
    }
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub(super) fn leaf_block(idx: &Index, leaf: &Leaf) -> Block {
    match leaf.kind {
        LeafKind::Para => inline_block(idx, leaf, BlockKind::Para, 0),
        LeafKind::Heading(level) => {
            let first = idx.line_text(leaf.first);
            let at_hash = first.chars().nth(leaf.content - idx.line_start(leaf.first)) == Some('#');
            if at_hash {
                inline_block(idx, leaf, BlockKind::Heading(level), level)
            } else {
                verbatim_block(idx, leaf, BlockKind::Verbatim, None)
            }
        }
        LeafKind::Code => verbatim_block(idx, leaf, BlockKind::Code, None),
        LeafKind::Table => verbatim_block(idx, leaf, BlockKind::Table, None),
        LeafKind::Html => verbatim_block(idx, leaf, BlockKind::Html, None),
        LeafKind::Meta => verbatim_block(idx, leaf, BlockKind::Verbatim, None),
        LeafKind::Rule => verbatim_block(idx, leaf, BlockKind::Rule, None),
    }
}

/// Parse the first-line prefix `[line start, upto)` into lead pieces.
pub(super) fn lead_pieces(idx: &Index, line: usize, upto: usize, item: bool) -> (Vec<Piece>, bool) {
    let start = idx.line_start(line);
    let chars: Vec<char> = idx
        .line_text(line)
        .chars()
        .take(upto.saturating_sub(start))
        .collect();
    let mut out = Vec::new();
    let mut done = false;
    let mut i = 0;
    let mut seen_marker = false;
    let piece = |a: usize, b: usize, kind: LeadKind| Piece {
        src: start + a..start + b,
        kind: PieceKind::Lead(kind),
        flags: 0,
    };
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            let mut j = i;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            out.push(piece(i, j, LeadKind::Indent));
            i = j;
        } else if c == '>' {
            let mut j = i + 1;
            if j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            out.push(piece(i, j, LeadKind::Quote));
            i = j;
        } else if item && !seen_marker && list_marker_len(&chars[i..]).is_some() {
            let (len, ordered) = list_marker_len(&chars[i..]).unwrap_or((1, false));
            out.push(piece(
                i,
                i + len,
                if ordered {
                    LeadKind::Number
                } else {
                    LeadKind::Bullet
                },
            ));
            seen_marker = true;
            i += len;
        } else if item && seen_marker && task_len(&chars[i..]).is_some() {
            let (len, checked) = task_len(&chars[i..]).unwrap_or((3, false));
            out.push(piece(i, i + len, LeadKind::Check(checked)));
            done = checked;
            i += len;
        } else {
            out.push(piece(i, chars.len(), LeadKind::Other));
            i = chars.len();
        }
    }
    (out, done)
}

/// `(len incl. trailing spaces, ordered)` of a list marker at the start of `s`.
pub(super) fn list_marker_len(s: &[char]) -> Option<(usize, bool)> {
    let mut i = 0;
    let ordered = s.first().is_some_and(char::is_ascii_digit);
    if ordered {
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 || !matches!(s.get(i), Some('.' | ')')) {
            return None;
        }
        i += 1;
    } else if matches!(s.first(), Some('-' | '*' | '+')) {
        i = 1;
    } else {
        return None;
    }
    if i < s.len() && s[i] != ' ' && s[i] != '\t' {
        return None;
    }
    while i < s.len() && (s[i] == ' ' || s[i] == '\t') {
        i += 1;
    }
    Some((i, ordered))
}

pub(super) fn task_len(s: &[char]) -> Option<(usize, bool)> {
    if s.len() >= 3 && s[0] == '[' && s[2] == ']' && matches!(s[1], ' ' | 'x' | 'X') {
        let mut i = 3;
        while i < s.len() && (s[i] == ' ' || s[i] == '\t') {
            i += 1;
        }
        Some((i, s[1] != ' '))
    } else {
        None
    }
}

/// Chars to hide at the start of a continuation line.
pub(super) fn strip_len(line: &str, quote: bool, limit: Option<usize>) -> usize {
    let mut n = 0;
    for c in line.chars() {
        if limit.is_some_and(|l| n >= l) {
            break;
        }
        if c == ' ' || c == '\t' || (quote && c == '>') {
            n += 1;
        } else {
            break;
        }
    }
    n
}

pub(super) fn inline_block(idx: &Index, leaf: &Leaf, kind: BlockKind, heading: u8) -> Block {
    let first_start = idx.line_start(leaf.first);
    let map_end = idx.line_end(leaf.last);
    let map = StyleMap::new(leaf, leaf.content, map_end);
    let (mut pieces, done) = lead_pieces(idx, leaf.first, leaf.content, leaf.first_in_item);
    if let Some(checked) = leaf.task {
        let _ = checked;
    }
    let mut body_start = leaf.content;
    if heading > 0 {
        // `## ` marks hang in the margin.
        let line: Vec<char> = idx.line_text(leaf.first).chars().collect();
        let mut i = leaf.content - first_start;
        let hashes = i;
        while i < line.len() && line[i] == '#' {
            i += 1;
        }
        while i < line.len() && line[i] == ' ' {
            i += 1;
        }
        let _ = hashes;
        pieces.push(Piece {
            src: first_start + (leaf.content - first_start)..first_start + i,
            kind: PieceKind::Lead(LeadKind::Heading),
            flags: 0,
        });
        body_start = first_start + i;
    }
    let quote = leaf.quote > 0;
    let cols = leaf.content.saturating_sub(first_start);
    let _ = cols;
    let mut line = leaf.first;
    let mut region_start = body_start;
    loop {
        let end = idx.line_end(line);
        let mut region_end = end;
        let last = line == leaf.last;
        let mut hard = false;
        if !last {
            let text: Vec<char> = idx.line_text(line).chars().collect();
            let from = region_start.saturating_sub(idx.line_start(line));
            let trailing = text.iter().rev().take_while(|&&c| c == ' ').count();
            if trailing >= 2 && text.len() - trailing > from {
                region_end = end - trailing;
                hard = true;
            }
        }
        if heading > 0 && last {
            // Closing `##`.
            let text: Vec<char> = idx.line_text(line).chars().collect();
            let from = region_start.saturating_sub(idx.line_start(line));
            let mut j = text.len();
            while j > from && text[j - 1] == ' ' {
                j -= 1;
            }
            let mut k = j;
            while k > from && text[k - 1] == '#' {
                k -= 1;
            }
            if k < j && k > from && text[k - 1] == ' ' {
                let cut = idx.line_start(line) + k - 1;
                styled_pieces(&map, region_start..cut, &mut pieces);
                pieces.push(Piece {
                    src: cut..end,
                    kind: PieceKind::Marker,
                    flags: 0,
                });
                break;
            }
        }
        styled_pieces(&map, region_start..region_end, &mut pieces);
        if last {
            break;
        }
        let next = line + 1;
        let strip = strip_len(idx.line_text(next), quote, None);
        let next_start = idx.line_start(next) + strip;
        pieces.push(Piece {
            src: region_end..next_start,
            kind: if hard {
                PieceKind::Hard
            } else {
                PieceKind::Soft
            },
            flags: 0,
        });
        line = next;
        region_start = next_start;
    }
    Block {
        src: first_start..map_end,
        kind,
        quote: leaf.quote,
        list: leaf.list,
        done,
        pieces,
        image: leaf.image.clone(),
        chip: None,
    }
}
