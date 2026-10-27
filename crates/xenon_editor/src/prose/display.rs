//! Prose projection, step two: one block's display text and the offset map
//! between buffer chars and display bytes. Pure (no gpui): layout shapes the
//! text, this module only says what the text is and where each source char
//! lands in it.

use std::ops::Range;

use super::model::{Block, BlockKind, Flags, LeadKind, PieceKind};

/// How a run of display text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunStyle {
    pub flags: Flags,
    /// Markers, fences, separators: the dim ink.
    pub dim: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Run {
    pub len: usize,
    pub style: RunStyle,
}

/// One hung item left of the body (quote marks, list marker, heading marks).
#[derive(Clone, Debug)]
pub struct LeadItem {
    pub src: Range<usize>,
    pub kind: LeadKind,
    /// Display text; empty for hidden items and the task box.
    pub text: String,
    /// Display chars equal source chars (caret can enter).
    pub one_to_one: bool,
}

#[derive(Clone, Debug)]
pub struct MapSeg {
    pub src: Range<usize>,
    pub dstart: usize,
    pub dlen: usize,
    pub one_to_one: bool,
}

/// Where a source offset is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// `byte` into `leads[item].text`.
    Lead(usize, usize),
    /// `byte` into the body text.
    Body(usize),
}

pub struct Display {
    pub leads: Vec<LeadItem>,
    pub body: String,
    pub runs: Vec<Run>,
    pub map: Vec<MapSeg>,
    /// Display lines (split on `\n`) that are a table separator hidden as a line.
    pub sep_lines: Vec<usize>,
}

impl Display {
    /// Build the display for `block`. `text_of` reads source chars.
    pub fn build(block: &Block, hide: bool, text_of: &dyn Fn(Range<usize>) -> String) -> Display {
        let mut out = Display {
            leads: Vec::new(),
            body: String::new(),
            runs: Vec::new(),
            map: Vec::new(),
            sep_lines: Vec::new(),
        };
        for piece in &block.pieces {
            let style = RunStyle {
                flags: piece.flags,
                dim: false,
            };
            let dim = RunStyle {
                flags: piece.flags,
                dim: true,
            };
            match piece.kind {
                PieceKind::Lead(kind) => out.lead(piece.src.clone(), kind, hide, text_of),
                PieceKind::Text => {
                    let t = text_of(piece.src.clone());
                    out.push(piece.src.clone(), &t, style, true);
                }
                PieceKind::Marker => {
                    if hide {
                        out.push(piece.src.clone(), "", dim, false);
                    } else {
                        let t = text_of(piece.src.clone());
                        out.push(piece.src.clone(), &t, dim, true);
                    }
                }
                PieceKind::Fence => {
                    let t = text_of(piece.src.clone());
                    out.push(piece.src.clone(), &t, dim, true);
                }
                PieceKind::TableSep => {
                    if hide {
                        let line = out.body.matches('\n').count();
                        out.sep_lines.push(line);
                        out.push(piece.src.clone(), "", dim, false);
                    } else {
                        let t = text_of(piece.src.clone());
                        out.push(piece.src.clone(), &t, dim, true);
                    }
                }
                PieceKind::Soft => out.push(piece.src.clone(), " ", style, false),
                PieceKind::Hard => {
                    if hide {
                        out.push(piece.src.clone(), "\n", style, false);
                    } else {
                        out.push(piece.src.clone(), "\u{21b5}\n", dim, false);
                    }
                }
                PieceKind::Prefix | PieceKind::Chip => {
                    out.push(piece.src.clone(), "", style, false)
                }
                PieceKind::Newline => out.push(piece.src.clone(), "\n", style, false),
            }
        }
        out
    }

    fn lead(
        &mut self,
        src: Range<usize>,
        kind: LeadKind,
        hide: bool,
        text_of: &dyn Fn(Range<usize>) -> String,
    ) {
        let raw = text_of(src.clone());
        let (text, one_to_one) = match kind {
            LeadKind::Indent => (String::new(), false),
            LeadKind::Check(_) => (String::new(), false),
            LeadKind::Quote | LeadKind::Heading if hide => (String::new(), false),
            LeadKind::Bullet if hide => ("\u{2022} ".to_string(), false),
            _ => (raw, true),
        };
        if matches!(kind, LeadKind::Check(_))
            && hide
            && let Some(bullet) = self
                .leads
                .iter_mut()
                .rev()
                .find(|l| l.kind == LeadKind::Bullet)
        {
            // Hide mode: only the box shows, not box plus bullet.
            bullet.text.clear();
        }
        self.leads.push(LeadItem {
            src,
            kind,
            text,
            one_to_one,
        });
    }

    fn push(&mut self, src: Range<usize>, text: &str, style: RunStyle, one_to_one: bool) {
        let dstart = self.body.len();
        self.body.push_str(text);
        if !text.is_empty() {
            match self.runs.last_mut() {
                Some(last) if last.style == style => last.len += text.len(),
                _ => self.runs.push(Run {
                    len: text.len(),
                    style,
                }),
            }
        }
        self.map.push(MapSeg {
            src,
            dstart,
            dlen: text.len(),
            one_to_one,
        });
    }

    /// Where source offset `pos` is drawn. Interior positions of collapsed
    /// segments snap to the nearer edge.
    pub fn locate(&self, block: &Block, pos: usize) -> Where {
        // Lead items first: a position strictly inside or at the start of a
        // lead belongs to it, except at its end (that is the body start).
        for (i, lead) in self.leads.iter().enumerate() {
            if pos >= lead.src.start && pos < lead.src.end {
                let byte = if lead.one_to_one {
                    char_to_byte(&lead.text, pos - lead.src.start)
                } else {
                    0
                };
                return Where::Lead(i, byte);
            }
        }
        // A position at the very end of the last lead of an empty body.
        let body_empty = self.map.is_empty();
        if body_empty {
            if let Some((i, lead)) = self
                .leads
                .iter()
                .enumerate()
                .rfind(|(_, l)| pos == l.src.end)
            {
                let byte = if lead.one_to_one { lead.text.len() } else { 0 };
                return Where::Lead(i, byte);
            }
            return Where::Body(0);
        }
        let _ = block;
        // Visible segment containing the position (start inclusive).
        for seg in &self.map {
            if seg.dlen > 0 && pos >= seg.src.start && pos < seg.src.end {
                let byte = if seg.one_to_one {
                    seg.dstart
                        + char_to_byte(
                            &self.body[seg.dstart..seg.dstart + seg.dlen],
                            pos - seg.src.start,
                        )
                } else if pos == seg.src.start {
                    seg.dstart
                } else {
                    seg.dstart + seg.dlen
                };
                return Where::Body(byte);
            }
        }
        // Edge cases: at a segment end (before hidden segments or at the end).
        let mut best: Option<usize> = None;
        for seg in &self.map {
            if pos == seg.src.end && seg.dlen > 0 {
                best = Some(seg.dstart + seg.dlen);
            }
            if pos == seg.src.start && seg.dlen > 0 {
                return Where::Body(seg.dstart);
            }
        }
        if let Some(b) = best {
            return Where::Body(b);
        }
        // Nothing visible touches the position: use the display point of the
        // nearest preceding segment.
        let mut at = 0;
        for seg in &self.map {
            if seg.src.start <= pos {
                at = seg.dstart + seg.dlen;
            }
        }
        Where::Body(at)
    }

    /// Source offset for a display position.
    pub fn src_at(&self, block: &Block, w: Where) -> usize {
        match w {
            Where::Lead(i, byte) => {
                let lead = &self.leads[i];
                if lead.one_to_one {
                    lead.src.start + lead.text[..byte.min(lead.text.len())].chars().count()
                } else if byte == 0 {
                    lead.src.start
                } else {
                    lead.src.end
                }
            }
            Where::Body(d) => {
                for seg in &self.map {
                    if seg.dlen > 0 && d >= seg.dstart && d < seg.dstart + seg.dlen {
                        return if seg.one_to_one {
                            seg.src.start
                                + self.body[seg.dstart..d.min(seg.dstart + seg.dlen)]
                                    .chars()
                                    .count()
                        } else if d == seg.dstart {
                            seg.src.start
                        } else {
                            seg.src.end
                        };
                    }
                }
                let mut best = None;
                for seg in &self.map {
                    if seg.dlen > 0 && d == seg.dstart + seg.dlen {
                        best = Some(seg.src.end);
                    }
                }
                if let Some(b) = best {
                    return b;
                }
                // Body is empty or only hidden: after the leads.
                self.leads
                    .last()
                    .map(|l| l.src.end)
                    .unwrap_or(block.src.start)
                    .min(block.src.end)
            }
        }
    }

    /// Whether the caret may rest at `pos` (not inside a collapsed segment).
    pub fn valid(&self, pos: usize) -> bool {
        let inside = |src: &Range<usize>| src.start < pos && pos < src.end;
        !self.leads.iter().any(|l| !l.one_to_one && inside(&l.src))
            && !self.map.iter().any(|s| !s.one_to_one && inside(&s.src))
    }
}

fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

impl BlockKind {
    /// Extra vertical gap (in lines) above a block of this kind.
    pub fn space_before(self) -> f32 {
        match self {
            BlockKind::Heading(1) => 0.5,
            BlockKind::Heading(2) => 0.45,
            BlockKind::Heading(_) => 0.35,
            _ => 0.0,
        }
    }
}
