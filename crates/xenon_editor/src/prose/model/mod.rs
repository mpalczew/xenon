//! Prose projection, step one: split the file text into display blocks.
//!
//! The file text is the only document. This module reads it (never writes it)
//! and describes, per block, how every source character appears: shown, dim,
//! collapsed (a soft line break becomes a space) or hidden (a continuation
//! prefix such as `> `). Offsets are char indices into the buffer; the
//! pulldown-cmark byte offsets are converted in exactly one place
//! ([`Index::byte_to_char`]).
//!
//! Every source line belongs to exactly one block, so every caret offset has a
//! home. Blocks never rewrite text; they only classify it.

use std::ops::Range;

pub type Flags = u8;
pub const BOLD: Flags = 1;
pub const ITALIC: Flags = 2;
pub const STRIKE: Flags = 4;
pub const CODE: Flags = 8;
pub const LINK: Flags = 16;
pub const URL: Flags = 32;
pub const FOOT: Flags = 64;
pub const HTML: Flags = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadKind {
    /// Leading whitespace of the first line: never drawn.
    Indent,
    /// `> ` quote marks, hung left of the quote rule.
    Quote,
    /// `- ` / `* ` / `+ `.
    Bullet,
    /// `1. `.
    Number,
    /// `[ ] ` / `[x] ` task box (atomic widget).
    Check(bool),
    /// `## ` heading marks.
    Heading,
    /// Anything else before the text (footnote definition label).
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    /// Shown, one display char per source char.
    Text,
    /// Inline syntax: dim, hidden in Hide mode.
    Marker,
    /// A soft line break plus the next line's prefix: displays as one space.
    Soft,
    /// Two-space hard break: displays as a dim return arrow and a line break.
    Hard,
    /// A continuation prefix inside a verbatim block: never drawn.
    Prefix,
    /// The `\n` between lines of a verbatim block.
    Newline,
    /// Text that comes before the body of the first line (hangs in the margin).
    Lead(LeadKind),
    /// Code fence, rule: dim text.
    Fence,
    /// Table separator row: dim text, a thin line in Hide mode.
    TableSep,
    /// The folded comment chip: atomic, covers the whole block.
    Chip,
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub src: Range<usize>,
    pub kind: PieceKind,
    pub flags: Flags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Blank,
    Para,
    Heading(u8),
    Code,
    Table,
    Html,
    Rule,
    /// Raw lines we do not style (front matter, setext, stray lines).
    Verbatim,
    /// The leading-comments chip (folded: covers the lines; open: empty).
    Chip,
}

impl BlockKind {
    /// Blocks whose lines are shown one-to-one in a monospace face.
    pub fn verbatim(self) -> bool {
        matches!(
            self,
            BlockKind::Code
                | BlockKind::Table
                | BlockKind::Html
                | BlockKind::Verbatim
                | BlockKind::Rule
        )
    }
}

#[derive(Clone, Debug)]
pub struct Chip {
    pub comments: usize,
    pub flags: usize,
    pub folded: bool,
}

#[derive(Clone, Debug)]
pub struct Block {
    /// Char range of the block's lines (without the final newline).
    pub src: Range<usize>,
    pub kind: BlockKind,
    pub quote: u8,
    pub list: u8,
    /// First block of a list item whose task box is checked.
    pub done: bool,
    pub pieces: Vec<Piece>,
    /// `![alt](src)` alone in a paragraph: the destination.
    pub image: Option<String>,
    pub chip: Option<Chip>,
}

pub struct Doc {
    pub blocks: Vec<Block>,
}

impl Doc {
    /// Parse `text`. `comments_open` unfolds the leading comment lines.
    pub fn parse(text: &str, comments_open: bool) -> Doc {
        let idx = Index::new(text);
        let (chip_lines, chip) = leading_comments(&idx);
        let leaves = walk(&idx, chip_lines);
        let mut owner: Vec<Option<usize>> = vec![None; idx.lines()];
        for (i, leaf) in leaves.iter().enumerate() {
            if owner[leaf.first..=leaf.last].iter().all(Option::is_none) {
                for slot in &mut owner[leaf.first..=leaf.last] {
                    *slot = Some(i);
                }
            } else {
                // Overlap: keep the first claim; this leaf is dropped.
                for slot in &mut owner[leaf.first..=leaf.last] {
                    slot.get_or_insert(usize::MAX);
                }
            }
        }
        let mut blocks = Vec::new();
        let mut line = 0;
        if let Some(chip) = chip.filter(|_| chip_lines > 0) {
            let end = idx.line_end(chip_lines - 1);
            let folded = !comments_open;
            if folded {
                blocks.push(Block {
                    src: 0..end,
                    kind: BlockKind::Chip,
                    quote: 0,
                    list: 0,
                    done: false,
                    pieces: vec![Piece {
                        src: 0..end,
                        kind: PieceKind::Chip,
                        flags: 0,
                    }],
                    image: None,
                    chip: Some(Chip { folded, ..chip }),
                });
            } else {
                blocks.push(Block {
                    src: 0..0,
                    kind: BlockKind::Chip,
                    quote: 0,
                    list: 0,
                    done: false,
                    pieces: Vec::new(),
                    image: None,
                    chip: Some(Chip { folded, ..chip }),
                });
                blocks.push(verbatim_block(
                    &idx,
                    &Leaf {
                        kind: LeafKind::Html,
                        first: 0,
                        last: chip_lines - 1,
                        content: 0,
                        ..Leaf::blank()
                    },
                    BlockKind::Html,
                    None,
                ));
            }
            line = chip_lines;
        }
        while line < idx.lines() {
            match owner[line] {
                Some(i) if i != usize::MAX && leaves[i].first == line => {
                    blocks.push(leaf_block(&idx, &leaves[i]));
                    line = leaves[i].last + 1;
                }
                _ => {
                    blocks.push(gap_block(&idx, line));
                    line += 1;
                }
            }
        }
        Doc { blocks }
    }

    /// Index of the block that owns char offset `pos`.
    pub fn block_at(&self, pos: usize) -> usize {
        let after = self.blocks.partition_point(|b| b.src.start <= pos);
        after.saturating_sub(1)
    }
}

/// Lines, char offsets and the one byte-to-char conversion.
pub struct Index<'a> {
    text: &'a str,
    line_byte: Vec<usize>,
    line_char: Vec<usize>,
}

impl<'a> Index<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_byte = vec![0];
        let mut line_char = vec![0];
        let mut chars = 0;
        for (byte, ch) in text.char_indices() {
            chars += 1;
            if ch == '\n' {
                line_byte.push(byte + 1);
                line_char.push(chars);
            }
        }
        Index {
            text,
            line_byte,
            line_char,
        }
    }

    pub fn lines(&self) -> usize {
        self.line_byte.len()
    }

    fn line_text(&self, line: usize) -> &'a str {
        let start = self.line_byte[line];
        let end = self
            .line_byte
            .get(line + 1)
            .map_or(self.text.len(), |next| next - 1);
        &self.text[start..end]
    }

    fn line_start(&self, line: usize) -> usize {
        self.line_char[line]
    }

    /// Char offset just before the line's newline.
    fn line_end(&self, line: usize) -> usize {
        self.line_char[line] + self.line_text(line).chars().count()
    }

    fn line_of_byte(&self, byte: usize) -> usize {
        self.line_byte
            .partition_point(|&b| b <= byte)
            .saturating_sub(1)
    }

    /// The only place parser bytes become buffer chars.
    pub fn byte_to_char(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        let line = self.line_of_byte(byte);
        self.line_char[line] + self.text[self.line_byte[line]..byte].chars().count()
    }
}

fn is_comment_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("<!--") && t.ends_with("-->") && t.len() >= 7
}

/// Count of leading `<!-- ... -->` lines and the chip describing them.
fn leading_comments(idx: &Index) -> (usize, Option<Chip>) {
    let mut count = 0;
    let mut flags = 0;
    while count < idx.lines() && is_comment_line(idx.line_text(count)) {
        if idx.line_text(count).contains("FLAG") {
            flags += 1;
        }
        count += 1;
    }
    if count == 0 {
        (0, None)
    } else {
        (
            count,
            Some(Chip {
                comments: count,
                flags,
                folded: true,
            }),
        )
    }
}

mod blocks;
mod verbatim;
mod walk;

use blocks::*;
use verbatim::*;
use walk::*;
