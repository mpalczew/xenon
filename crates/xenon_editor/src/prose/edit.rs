//! Prose editing rules. Pure: given the rope, the projection and a caret, say
//! which plain source edit a key means. Nothing here rewrites untouched text.

use std::ops::Range;

use ropey::Rope;

use super::model::{Block, BlockKind, LeadKind, PieceKind};
use super::projection::Projection;

/// Replace `range` (chars) with `text`; the caret lands after the text.
pub struct Plan {
    pub range: Range<usize>,
    pub text: String,
}

fn slice(rope: &Rope, r: Range<usize>) -> String {
    rope.slice(r.start..r.end.min(rope.len_chars())).to_string()
}

/// First-line lead pieces of a block as `(kind, src)`.
fn leads(block: &Block) -> Vec<(LeadKind, Range<usize>)> {
    block
        .pieces
        .iter()
        .filter_map(|p| match p.kind {
            PieceKind::Lead(k) => Some((k, p.src.clone())),
            _ => None,
        })
        .collect()
}

fn has_soft(block: &Block) -> bool {
    block
        .pieces
        .iter()
        .any(|p| matches!(p.kind, PieceKind::Soft | PieceKind::Hard))
}

fn bump_number(raw: &str) -> String {
    let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
    let rest = &raw[digits.len()..];
    let n: u64 = digits.parse().unwrap_or(0);
    format!("{}{rest}", n + 1)
}

/// Enter (`shift` false) or Shift+Enter (`shift` true) at `pos`.
pub fn enter(rope: &Rope, proj: &Projection, pos: usize, shift: bool) -> Plan {
    let block = proj.block(proj.doc.block_at(pos));
    let at = |text: &str| Plan {
        range: pos..pos,
        text: text.to_string(),
    };
    match block.kind {
        BlockKind::Code
        | BlockKind::Table
        | BlockKind::Html
        | BlockKind::Verbatim
        | BlockKind::Rule
        | BlockKind::Chip => return at("\n"),
        BlockKind::Blank if block.quote == 0 && block.list == 0 => return at("\n"),
        _ => {}
    }
    let ls = leads(block);
    let body_start = ls.last().map_or(block.src.start, |(_, r)| r.end);
    if ls.is_empty() || pos < body_start {
        if matches!(block.kind, BlockKind::Para | BlockKind::Heading(_)) && !shift {
            return at("\n\n");
        }
        return at("\n");
    }
    let is_heading = matches!(block.kind, BlockKind::Heading(_));
    if is_heading {
        return at(if shift { "\n" } else { "\n\n" });
    }
    let list = ls
        .iter()
        .any(|(k, _)| matches!(k, LeadKind::Bullet | LeadKind::Number | LeadKind::Check(_)));
    let quote_only = !list && ls.iter().any(|(k, _)| *k == LeadKind::Quote);
    let empty = block.src.end == body_start && !has_soft(block);
    if empty && pos == body_start {
        // Enter on an empty item or quote line removes its marker.
        let from = ls
            .iter()
            .find(|(k, _)| {
                matches!(
                    k,
                    LeadKind::Bullet | LeadKind::Number | LeadKind::Check(_) | LeadKind::Quote
                )
            })
            .map_or(body_start, |(_, r)| r.start);
        let from = ls
            .iter()
            .rev()
            .find(|(k, r)| *k == LeadKind::Indent && r.end == from)
            .map_or(from, |(_, r)| r.start);
        return Plan {
            range: from..block.src.end,
            text: String::new(),
        };
    }
    let mut cont = String::new();
    let mut next = String::new();
    for (kind, r) in &ls {
        let raw = slice(rope, r.clone());
        match kind {
            LeadKind::Indent | LeadKind::Quote => {
                cont.push_str(&raw);
                next.push_str(&raw);
            }
            LeadKind::Bullet => {
                cont.push_str(&" ".repeat(raw.chars().count()));
                next.push_str(&raw);
            }
            LeadKind::Number => {
                cont.push_str(&" ".repeat(raw.chars().count()));
                next.push_str(&bump_number(&raw));
            }
            LeadKind::Check(_) => {
                cont.push_str(&" ".repeat(raw.chars().count()));
                let mut c: Vec<char> = raw.chars().collect();
                if c.len() > 1 {
                    c[1] = ' ';
                }
                next.extend(c);
            }
            LeadKind::Heading | LeadKind::Other => {}
        }
    }
    if shift {
        return at(&format!("\n{cont}"));
    }
    if quote_only {
        let bare = cont.trim_end().to_string();
        return at(&format!("\n{bare}\n{cont}"));
    }
    at(&format!("\n{next}"))
}

/// Backspace at `pos` (no selection).
pub fn backspace(rope: &Rope, proj: &Projection, pos: usize) -> Option<Range<usize>> {
    if pos == 0 {
        return None;
    }
    let block = proj.block(proj.doc.block_at(pos));
    let ls = leads(block);
    // Caret right after a marker: remove the whole marker.
    if let Some((_, r)) = ls
        .iter()
        .rev()
        .find(|(k, r)| r.end == pos && r.end > r.start && *k != LeadKind::Other)
        && pos > block.src.start
    {
        return Some(r.start..pos);
    }
    // Chip: delete the whole chip as one.
    if block.kind == BlockKind::Chip && block.src.end > block.src.start && pos == block.src.end {
        return Some(block.src.clone());
    }
    let mut start = pos - 1;
    while start > 0 && !proj.valid(start) {
        start -= 1;
    }
    let _ = rope;
    Some(start..pos)
}

/// Delete at `pos` (no selection).
pub fn delete(rope: &Rope, proj: &Projection, pos: usize) -> Option<Range<usize>> {
    let len = rope.len_chars();
    if pos >= len {
        return None;
    }
    let mut end = pos + 1;
    while end < len && !proj.valid(end) {
        end += 1;
    }
    Some(pos..end)
}

/// The one-char edit that flips the task box in the block at `pos`.
pub fn toggle_task(rope: &Rope, proj: &Projection, pos: usize) -> Option<Plan> {
    let block = proj.block(proj.doc.block_at(pos));
    let (_, r) = leads(block)
        .into_iter()
        .find(|(k, _)| matches!(k, LeadKind::Check(_)))?;
    toggle_at(rope, r.start)
}

/// Flip the box whose `[` is at `open`.
pub fn toggle_at(rope: &Rope, open: usize) -> Option<Plan> {
    let flip = if slice(rope, open + 1..open + 2) == " " {
        "x"
    } else {
        " "
    };
    Some(Plan {
        range: open + 1..open + 2,
        text: flip.to_string(),
    })
}
