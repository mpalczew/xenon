//! Model step three: verbatim blocks (code, tables, html) and gap lines.

use std::ops::Range;

use super::*;

pub(super) fn is_table_sep(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

pub(super) fn verbatim_block(
    idx: &Index,
    leaf: &Leaf,
    kind: BlockKind,
    _unused: Option<()>,
) -> Block {
    let first_start = idx.line_start(leaf.first);
    let end = idx.line_end(leaf.last);
    let (mut pieces, done) = lead_pieces(idx, leaf.first, leaf.content, leaf.first_in_item);
    let cols = leaf.content.saturating_sub(first_start);
    for line in leaf.first..=leaf.last {
        let start = idx.line_start(line);
        let text = idx.line_text(line);
        let (from, _) = if line == leaf.first {
            (leaf.content, ())
        } else {
            let strip = strip_len(text, leaf.quote > 0, Some(cols));
            if strip > 0 {
                pieces.push(Piece {
                    src: start..start + strip,
                    kind: PieceKind::Prefix,
                    flags: 0,
                });
            }
            (start + strip, ())
        };
        let to = idx.line_end(line);
        if from < to {
            let body: String = text.chars().skip(from - start).collect();
            line_pieces(kind, leaf, line, &body, from..to, &mut pieces);
        }
        if line < leaf.last {
            pieces.push(Piece {
                src: to..to + 1,
                kind: PieceKind::Newline,
                flags: 0,
            });
        }
    }
    Block {
        src: first_start..end,
        kind,
        quote: leaf.quote,
        list: leaf.list,
        done,
        pieces,
        image: None,
        chip: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn line_pieces(
    kind: BlockKind,
    leaf: &Leaf,
    line: usize,
    body: &str,
    range: Range<usize>,
    out: &mut Vec<Piece>,
) {
    match kind {
        BlockKind::Code => {
            let trimmed = body.trim_start();
            let fence = leaf.fenced
                && (trimmed.starts_with("```") || trimmed.starts_with("~~~"))
                && (line == leaf.first || line == leaf.last);
            out.push(Piece {
                src: range,
                kind: if fence {
                    PieceKind::Fence
                } else {
                    PieceKind::Text
                },
                flags: CODE,
            });
        }
        BlockKind::Table => {
            if is_table_sep(body) {
                out.push(Piece {
                    src: range,
                    kind: PieceKind::TableSep,
                    flags: 0,
                });
                return;
            }
            let mut pos = range.start;
            let mut run_start = pos;
            for ch in body.chars() {
                if ch == '|' {
                    if run_start < pos {
                        out.push(Piece {
                            src: run_start..pos,
                            kind: PieceKind::Text,
                            flags: 0,
                        });
                    }
                    out.push(Piece {
                        src: pos..pos + 1,
                        kind: PieceKind::Marker,
                        flags: 0,
                    });
                    run_start = pos + 1;
                }
                pos += 1;
            }
            if run_start < pos {
                out.push(Piece {
                    src: run_start..pos,
                    kind: PieceKind::Text,
                    flags: 0,
                });
            }
        }
        BlockKind::Rule => out.push(Piece {
            src: range,
            kind: PieceKind::Fence,
            flags: 0,
        }),
        _ => out.push(Piece {
            src: range,
            kind: PieceKind::Text,
            flags: HTML,
        }),
    }
}

/// A line no leaf owns: blank, a lone `>` quote line, or stray text.
pub(super) fn gap_block(idx: &Index, line: usize) -> Block {
    let start = idx.line_start(line);
    let end = idx.line_end(line);
    let text = idx.line_text(line);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut quote = 0u8;
    while i < chars.len() && (chars[i] == '>' || chars[i] == ' ') {
        if chars[i] == '>' {
            quote += 1;
        }
        i += 1;
    }
    if quote == 0 {
        i = 0;
    }
    let rest: String = chars[i..].iter().collect();
    let blank = rest.trim().is_empty();
    // A list marker with no text yet ("- ", "3. "): an empty item.
    let probe: Vec<char> = rest.trim_start().chars().collect();
    let empty_item = !blank
        && list_marker_len(&probe)
            .is_some_and(|(len, _)| probe[len..].iter().all(|c| c.is_whitespace()));
    if empty_item {
        let (pieces, _) = lead_pieces(idx, line, end, true);
        return Block {
            src: start..end,
            kind: BlockKind::Para,
            quote,
            list: 1,
            done: false,
            pieces,
            image: None,
            chip: None,
        };
    }
    let mut pieces = Vec::new();
    if quote > 0 {
        pieces.push(Piece {
            src: start..start + i,
            kind: PieceKind::Lead(LeadKind::Quote),
            flags: 0,
        });
    }
    if start + i < end {
        pieces.push(Piece {
            src: start + i..end,
            kind: PieceKind::Text,
            flags: if blank { 0 } else { HTML },
        });
    }
    Block {
        src: start..end,
        kind: if blank {
            BlockKind::Blank
        } else {
            BlockKind::Verbatim
        },
        quote,
        list: 0,
        done: false,
        pieces,
        image: None,
        chip: None,
    }
}
