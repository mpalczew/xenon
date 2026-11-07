//! Markdown to style spans for a prose-mode text view. No UI dependency: a
//! frontend turns the spans into its own text tags and decides, from the
//! caret, which markup to hide. All offsets are `char` offsets, which is what
//! GTK text buffers use.
//!
//! It parses with pulldown-cmark: markup is drawn dim, or hidden except in the
//! block the caret is in. It only describes styled ranges; it never lays
//! anything out.

use std::ops::Range;

mod conv;
mod parse;
mod query;

/// What a styled range is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A heading line (level 1 to 6), markup included.
    Heading(u8),
    Bold,
    Italic,
    Strike,
    /// Inline code, backticks included.
    Code,
    /// A fenced or indented code block, fences included.
    CodeBlock,
    /// A block quote, `>` marks included.
    Quote,
    /// First line of a list item at nesting depth 0 to 3.
    ListItem(u8),
    /// The bullet or number.
    ListMarker,
    /// The `[ ]` or `[x]` box of a task item.
    Checkbox(bool),
    /// The first line of a finished task item.
    Done,
    /// Link text.
    Link,
    /// Image alt text.
    ImageAlt,
    Rule,
    /// Tables and HTML blocks: shown as written, in the code face.
    Verbatim,
    /// Front matter.
    Meta,
}

/// One styled range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub kind: Kind,
}

/// Syntax characters that can be hidden: `**`, `# `, backticks, `[`, `](url)`,
/// fences, `>` marks. `block` is the leaf block they belong to; they show
/// while the caret is in that block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markup {
    pub range: Range<usize>,
    pub block: Option<usize>,
}

/// A link's text range and where it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRef {
    pub range: Range<usize>,
    pub url: String,
}

/// An image: its whole `![alt](url)` range and the end of its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub range: Range<usize>,
    pub line_end: usize,
    pub url: String,
}

/// A task box and the first line of its item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkbox {
    /// The `[ ]` itself, three chars.
    pub range: Range<usize>,
    pub line: Range<usize>,
    pub checked: bool,
}

/// A parsed document.
#[derive(Clone, Debug, Default)]
pub struct Doc {
    /// Sorted by start; ranges may overlap and nest.
    pub spans: Vec<Span>,
    /// Sorted by start; never overlap.
    pub markup: Vec<Markup>,
    /// Leaf blocks (paragraph, heading, code block, item text), sorted and
    /// disjoint, expanded to whole lines.
    pub blocks: Vec<Range<usize>>,
    pub links: Vec<LinkRef>,
    pub images: Vec<ImageRef>,
    pub checkboxes: Vec<Checkbox>,
}

impl Doc {
    pub fn parse(text: &str) -> Doc {
        parse::parse(text)
    }

    /// Index of the leaf block the caret at `pos` is in, if any.
    pub fn block_at(&self, pos: usize) -> Option<usize> {
        let ix = self.blocks.partition_point(|b| b.start <= pos);
        let ix = ix.checked_sub(1)?;
        (pos <= self.blocks[ix].end).then_some(ix)
    }

    /// Whether the markup at index `ix` is drawn (not hidden) with the caret at
    /// `pos`. Hidden markup that belongs to no block never shows.
    pub fn markup_shown(&self, ix: usize, caret_block: Option<usize>) -> bool {
        caret_block.is_some() && self.markup[ix].block == caret_block
    }

    /// Indices of the markup that lies inside `range`.
    pub fn markup_in(&self, range: &Range<usize>) -> Range<usize> {
        let a = self.markup.partition_point(|m| m.range.end <= range.start);
        let b = self.markup.partition_point(|m| m.range.start < range.end);
        a..b.max(a)
    }

    /// Keep offsets right after an edit: `removed` chars at `at` went away and
    /// `inserted` chars took their place.
    pub fn shift(&mut self, at: usize, removed: usize, inserted: usize) {
        let map = |p: usize| -> usize {
            if p <= at {
                p
            } else if p >= at + removed {
                p - removed + inserted
            } else {
                at + inserted
            }
        };
        let fix = |r: &mut Range<usize>| *r = map(r.start)..map(r.end);
        self.spans.iter_mut().for_each(|s| fix(&mut s.range));
        self.markup.iter_mut().for_each(|m| fix(&mut m.range));
        self.blocks.iter_mut().for_each(fix);
        self.links.iter_mut().for_each(|l| fix(&mut l.range));
        for image in &mut self.images {
            fix(&mut image.range);
            image.line_end = map(image.line_end);
        }
        for c in &mut self.checkboxes {
            fix(&mut c.range);
            fix(&mut c.line);
        }
    }
}
