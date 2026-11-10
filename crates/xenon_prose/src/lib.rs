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
mod enter;
mod parse;
mod query;

pub use enter::{Enter, list_enter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Heading(u8),
    Bold,
    Italic,
    Strike,
    Code,
    CodeBlock,
    Quote,
    ListItem(u8),
    ListMarker,
    Checkbox(bool),
    Done,
    Link,
    ImageAlt,
    Rule,
    Verbatim,
    Meta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markup {
    pub range: Range<usize>,
    pub block: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRef {
    pub range: Range<usize>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub range: Range<usize>,
    pub line_end: usize,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkbox {
    pub range: Range<usize>,
    pub line: Range<usize>,
    pub checked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub line: Range<usize>,
    pub pipes: Vec<usize>,
    pub top: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeRef {
    pub lang: String,
    pub body: Range<usize>,
}

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
    /// The `-`, `*` or `+` of bullet items, sorted; shown as a round bullet
    /// away from the caret. `block` is the item's text block.
    pub bullets: Vec<Markup>,
    pub rows: Vec<Row>,
    pub code: Vec<CodeRef>,
}

impl Doc {
    pub fn parse(text: &str) -> Doc {
        parse::parse(text)
    }

    pub fn block_at(&self, pos: usize) -> Option<usize> {
        let ix = self.blocks.partition_point(|b| b.start <= pos);
        let ix = ix.checked_sub(1)?;
        (pos <= self.blocks[ix].end).then_some(ix)
    }

    /// Whether the markup at index `ix` is drawn (not hidden) with the caret in
    /// `caret_block`. Hidden markup that belongs to no block never shows.
    pub fn markup_shown(&self, ix: usize, caret_block: Option<usize>) -> bool {
        caret_block.is_some() && self.markup[ix].block == caret_block
    }

    pub fn markup_in(&self, range: &Range<usize>) -> Range<usize> {
        let a = self.markup.partition_point(|m| m.range.end <= range.start);
        let b = self.markup.partition_point(|m| m.range.start < range.end);
        a..b.max(a)
    }

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
        self.bullets.iter_mut().for_each(|m| fix(&mut m.range));
        self.code.iter_mut().for_each(|c| fix(&mut c.body));
        for row in &mut self.rows {
            fix(&mut row.line);
            row.pipes.iter_mut().for_each(|p| *p = map(*p));
        }
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

pub fn column_width(text: &str) -> f32 {
    const DEFAULT: f32 = 66.0;
    let mut lens = Vec::new();
    let mut fence = false;
    let mut lines = text.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for line in lines.by_ref() {
            if line.trim_end() == "---" {
                break;
            }
        }
    }
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        let skip = fence || line.starts_with("    ") || line.starts_with('\t');
        let markup = t.starts_with(['#', '>', '|', '<']) || t.contains("](");
        let next = lines.peek().map_or("", |l| l.trim());
        let continues = !next.is_empty()
            && !next.starts_with(['#', '>', '|', '<', '`', '~'])
            && !starts_list_item(next);
        let len = line.trim_end().chars().count();
        if !skip && !markup && continues && len >= 30 {
            lens.push(len);
        }
    }
    if lens.len() < 3 {
        return DEFAULT;
    }
    lens.sort_unstable();
    let p90 = lens[lens.len() * 9 / 10];
    let median = lens[lens.len() / 2];
    if median * 5 >= p90 * 4 {
        (p90 + p90 / 10 + 2).min(120) as f32
    } else {
        DEFAULT
    }
}

fn starts_list_item(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}
