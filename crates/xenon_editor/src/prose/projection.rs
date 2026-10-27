//! The projection: parsed blocks plus their displays, and caret stepping that
//! respects collapsed/hidden source. Pure; layout and edit logic both read it.

use ropey::Rope;

use super::display::{Display, Where};
use super::model::{Block, BlockKind, Doc};

pub struct Projection {
    pub doc: Doc,
    pub displays: Vec<Display>,
    pub hide: bool,
}

impl Projection {
    pub fn build(rope: &Rope, comments_open: bool, hide: bool) -> Projection {
        let text = rope.to_string();
        let doc = Doc::parse(&text, comments_open);
        let text_of = |range: std::ops::Range<usize>| -> String {
            let len = rope.len_chars();
            rope.slice(range.start.min(len)..range.end.min(len))
                .to_string()
        };
        let displays = doc
            .blocks
            .iter()
            .map(|b| Display::build(b, hide, &text_of))
            .collect();
        Projection {
            doc,
            displays,
            hide,
        }
    }

    pub fn block(&self, ix: usize) -> &Block {
        &self.doc.blocks[ix]
    }

    /// Block index and display position of buffer offset `pos`.
    pub fn locate(&self, pos: usize) -> (usize, Where) {
        let ix = self.doc.block_at(pos);
        (ix, self.displays[ix].locate(&self.doc.blocks[ix], pos))
    }

    pub fn valid(&self, pos: usize) -> bool {
        let ix = self.doc.block_at(pos);
        let block = &self.doc.blocks[ix];
        if block.kind == BlockKind::Chip && block.src.end > block.src.start {
            return pos == block.src.start || pos == block.src.end;
        }
        self.displays[ix].valid(pos)
    }

    /// One caret stop left (`-1`) or right (`1`) of `pos`, skipping positions
    /// inside collapsed text and (with markers hidden) stops that draw at the
    /// same place.
    pub fn step(&self, pos: usize, dir: i32, len: usize) -> usize {
        let here = self.locate(pos);
        let mut p = pos;
        loop {
            let next = if dir < 0 {
                if p == 0 {
                    return p;
                }
                p - 1
            } else {
                if p >= len {
                    return p;
                }
                p + 1
            };
            p = next;
            if !self.valid(p) {
                continue;
            }
            if self.hidden_stop(p, here) {
                continue;
            }
            return p;
        }
    }

    /// A stop that draws exactly where the caret already is (hidden markers).
    fn hidden_stop(&self, p: usize, here: (usize, Where)) -> bool {
        self.hide && self.locate(p) == here
    }

    /// Words in the prose (paragraphs, headings, tables; not code or markup).
    pub fn words(&self) -> usize {
        self.doc
            .blocks
            .iter()
            .zip(&self.displays)
            .filter(|(b, _)| {
                matches!(
                    b.kind,
                    BlockKind::Para | BlockKind::Heading(_) | BlockKind::Table
                )
            })
            .map(|(_, d)| {
                d.body
                    .split_whitespace()
                    .filter(|w| w.chars().any(char::is_alphanumeric))
                    .count()
            })
            .sum()
    }
}
