//! Model step one: pulldown events to leaves with inline spans.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeafKind {
    Para,
    Heading(u8),
    Code,
    Table,
    Html,
    Rule,
    Meta,
}

#[derive(Clone, Debug)]
pub(super) struct Leaf {
    pub(super) kind: LeafKind,
    pub(super) first: usize,
    pub(super) last: usize,
    /// Char offset where the parser says the content starts.
    pub(super) content: usize,
    pub(super) quote: u8,
    pub(super) list: u8,
    pub(super) first_in_item: bool,
    pub(super) task: Option<bool>,
    pub(super) spans: Vec<Span>,
    pub(super) image: Option<String>,
    pub(super) fenced: bool,
}

impl Leaf {
    pub(super) fn blank() -> Leaf {
        Leaf {
            kind: LeafKind::Para,
            first: 0,
            last: 0,
            content: 0,
            quote: 0,
            list: 0,
            first_in_item: false,
            task: None,
            spans: Vec::new(),
            image: None,
            fenced: false,
        }
    }
}

/// An inline style over a char range, with the delimiter ranges inside it.
#[derive(Clone, Debug)]
pub(super) struct Span {
    pub(super) range: Range<usize>,
    pub(super) flags: Flags,
    pub(super) markers: Vec<Range<usize>>,
    /// Marker ranges that are also a link URL.
    pub(super) url: bool,
}

pub(super) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

pub(super) struct OpenInline {
    pub(super) tag: Tag<'static>,
    pub(super) range: Range<usize>,
    pub(super) children: Option<Range<usize>>,
}

/// Run the parser and collect leaf blocks with their container context.
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub(super) fn walk(idx: &Index, skip_lines: usize) -> Vec<Leaf> {
    let mut leaves: Vec<Leaf> = Vec::new();
    let mut quote = 0u8;
    let mut list = 0u8;
    let mut item_pending = false;
    let mut task: Option<bool> = None;
    let mut cur: Option<Leaf> = None;
    let mut cur_end = 0usize;
    let mut explicit = false;
    let mut open: Vec<OpenInline> = Vec::new();
    let mut skip = 0u32;
    let mut pending_image: Option<(usize, usize, String)> = None;

    let parser = Parser::new_ext(idx.text, options()).into_offset_iter();
    for (event, range) in parser {
        let crange = idx.byte_to_char(range.start)..idx.byte_to_char(range.end);
        if skip > 0 {
            match event {
                Event::Start(_) => skip += 1,
                Event::End(_) => skip -= 1,
                _ => {}
            }
            continue;
        }
        macro_rules! close {
            () => {
                if let Some(mut leaf) = cur.take() {
                    if !explicit {
                        finish_lines(idx, &mut leaf, cur_end);
                    }
                    finish_image(&mut leaf, pending_image.take());
                    leaves.push(leaf);
                }
            };
        }
        macro_rules! begin {
            ($kind:expr, $range:expr, $explicit:expr) => {{
                close!();
                let mut leaf = Leaf::blank();
                leaf.kind = $kind;
                leaf.quote = quote;
                leaf.list = list;
                leaf.first_in_item = std::mem::take(&mut item_pending);
                leaf.task = task.take().filter(|_| leaf.first_in_item);
                leaf.content = $range.start;
                leaf.first = line_of_char(idx, $range.start);
                leaf.last = leaf.first;
                if $explicit {
                    leaf.last = last_line_of(idx, $range.start, $range.end);
                }
                cur_end = $range.end;
                explicit = $explicit;
                cur = Some(leaf);
            }};
        }
        match event {
            Event::Start(Tag::BlockQuote(_)) => {
                close!();
                quote = quote.saturating_add(1);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                close!();
                quote = quote.saturating_sub(1);
            }
            Event::Start(Tag::List(_)) => {
                close!();
                list = list.saturating_add(1);
            }
            Event::End(TagEnd::List(_)) => {
                close!();
                list = list.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                close!();
                item_pending = true;
            }
            Event::End(TagEnd::Item) => close!(),
            Event::Start(Tag::Paragraph) => begin!(LeafKind::Para, crange, true),
            Event::End(TagEnd::Paragraph) => close!(),
            Event::Start(Tag::Heading { level, .. }) => {
                begin!(LeafKind::Heading(level as u8), crange, true)
            }
            Event::End(TagEnd::Heading(_)) => close!(),
            Event::Start(Tag::CodeBlock(kind)) => {
                begin!(LeafKind::Code, crange, true);
                if let Some(leaf) = cur.as_mut() {
                    leaf.fenced = matches!(kind, CodeBlockKind::Fenced(_));
                }
                close!();
                skip = 1;
            }
            Event::Start(Tag::HtmlBlock) => {
                begin!(LeafKind::Html, crange, true);
                close!();
                skip = 1;
            }
            Event::Start(Tag::Table(_)) => {
                begin!(LeafKind::Table, crange, true);
                close!();
                skip = 1;
            }
            Event::Start(Tag::MetadataBlock(_)) => {
                begin!(LeafKind::Meta, crange, true);
                close!();
                skip = 1;
            }
            Event::Rule => {
                begin!(LeafKind::Rule, crange, true);
                close!();
            }
            Event::TaskListMarker(checked) => task = Some(checked),
            Event::Start(Tag::FootnoteDefinition(_)) | Event::End(TagEnd::FootnoteDefinition) => {
                close!()
            }
            other => {
                if !is_inline(&other) {
                    continue;
                }
                if cur.is_none() {
                    begin!(LeafKind::Para, crange, false);
                }
                cur_end = cur_end.max(crange.end);
                if let Some(leaf) = cur.as_mut() {
                    inline_event(idx, leaf, &mut open, &mut pending_image, other, crange);
                }
            }
        }
    }
    if let Some(mut leaf) = cur.take() {
        if !explicit {
            finish_lines(idx, &mut leaf, cur_end);
        }
        finish_image(&mut leaf, pending_image.take());
        leaves.push(leaf);
    }
    leaves.retain(|leaf| leaf.first >= skip_lines && leaf.last < idx.lines());
    leaves
}

pub(super) fn is_inline(event: &Event) -> bool {
    match event {
        Event::Text(_)
        | Event::Code(_)
        | Event::InlineHtml(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::FootnoteReference(_)
        | Event::InlineMath(_)
        | Event::DisplayMath(_) => true,
        Event::Start(tag) => matches!(
            tag,
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
        ),
        Event::End(tag) => matches!(
            tag,
            TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image
        ),
        _ => false,
    }
}

pub(super) fn line_of_char(idx: &Index, ch: usize) -> usize {
    idx.line_char
        .partition_point(|&c| c <= ch)
        .saturating_sub(1)
        .min(idx.lines() - 1)
}

pub(super) fn last_line_of(idx: &Index, start: usize, end: usize) -> usize {
    let first = line_of_char(idx, start);
    if end <= start {
        return first;
    }
    // A range that ends right after a newline does not own the next line.
    line_of_char(idx, end - 1).max(first)
}

pub(super) fn finish_lines(idx: &Index, leaf: &mut Leaf, end: usize) {
    leaf.last = last_line_of(idx, leaf.content, end.max(leaf.content));
}

pub(super) fn finish_image(leaf: &mut Leaf, image: Option<(usize, usize, String)>) {
    if let Some((start, end, dest)) = image
        && start == leaf.content
        && leaf.first == leaf.last
    {
        let _ = end;
        leaf.image = Some(dest);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inline_event(
    idx: &Index,
    leaf: &mut Leaf,
    open: &mut Vec<OpenInline>,
    image: &mut Option<(usize, usize, String)>,
    event: Event,
    range: Range<usize>,
) {
    match event {
        Event::Start(tag) => open.push(OpenInline {
            tag: tag.into_static(),
            range,
            children: None,
        }),
        Event::End(_) => {
            let Some(done) = open.pop() else { return };
            if let Some(parent) = open.last_mut() {
                grow(&mut parent.children, &done.range);
            }
            push_span(idx, leaf, &done, image);
        }
        Event::Code(_) => {
            if let Some(parent) = open.last_mut() {
                grow(&mut parent.children, &range);
            }
            let text = idx.text;
            let _ = text;
            leaf.spans.push(code_span(idx, range));
        }
        Event::FootnoteReference(_) => {
            #[allow(clippy::single_range_in_vec_init)]
            let mut markers = vec![range.start..(range.start + 2).min(range.end)];
            if range.end > range.start + 2 {
                markers.push(range.end - 1..range.end);
            }
            leaf.spans.push(Span {
                range,
                flags: FOOT,
                markers,
                url: false,
            });
        }
        Event::InlineHtml(_) => leaf.spans.push(Span {
            range,
            flags: HTML,
            markers: Vec::new(),
            url: false,
        }),
        Event::Text(_) => {
            if let Some(parent) = open.last_mut() {
                grow(&mut parent.children, &range);
            }
        }
        _ => {}
    }
}

pub(super) fn grow(slot: &mut Option<Range<usize>>, range: &Range<usize>) {
    *slot = Some(match slot.take() {
        Some(r) => r.start.min(range.start)..r.end.max(range.end),
        None => range.clone(),
    });
}

/// A code span: backtick runs at both ends are markers.
pub(super) fn code_span(idx: &Index, range: Range<usize>) -> Span {
    let src: Vec<char> = char_slice(idx, range.clone());
    let lead = src.iter().take_while(|&&c| c == '`').count();
    let trail = src.iter().rev().take_while(|&&c| c == '`').count();
    let (lead, trail) = if lead + trail > src.len() {
        (0, 0)
    } else {
        (lead, trail)
    };
    let mut markers = Vec::new();
    if lead > 0 {
        markers.push(range.start..range.start + lead);
    }
    if trail > 0 {
        markers.push(range.end - trail..range.end);
    }
    Span {
        range,
        flags: CODE,
        markers,
        url: false,
    }
}

pub(super) fn char_slice(idx: &Index, range: Range<usize>) -> Vec<char> {
    let line = line_of_char(idx, range.start);
    let mut out = Vec::new();
    let mut pos = idx.line_start(line);
    let mut l = line;
    'outer: loop {
        for ch in idx.line_text(l).chars() {
            if pos >= range.end {
                break 'outer;
            }
            if pos >= range.start {
                out.push(ch);
            }
            pos += 1;
        }
        // The newline itself.
        if l + 1 >= idx.lines() || pos >= range.end {
            break;
        }
        if pos >= range.start {
            out.push('\n');
        }
        pos += 1;
        l += 1;
    }
    out
}

pub(super) fn push_span(
    idx: &Index,
    leaf: &mut Leaf,
    done: &OpenInline,
    image: &mut Option<(usize, usize, String)>,
) {
    let range = done.range.clone();
    match &done.tag {
        Tag::Strong => leaf.spans.push(delimited(idx, range, BOLD, 2)),
        Tag::Emphasis => leaf.spans.push(delimited(idx, range, ITALIC, 1)),
        Tag::Strikethrough => {
            let src = char_slice(idx, range.clone());
            let n = src.iter().take_while(|&&c| c == '~').count().clamp(1, 2);
            leaf.spans.push(delimited(idx, range, STRIKE, n));
        }
        Tag::Link { .. } | Tag::Image { .. } => {
            let (ts, te) = match &done.children {
                Some(c) => (c.start.max(range.start), c.end.min(range.end)),
                None => (range.start, range.start),
            };
            let mut markers = Vec::new();
            if ts > range.start {
                markers.push(range.start..ts);
            }
            if te < range.end {
                markers.push(te..range.end);
            }
            if let Tag::Image { dest_url, .. } = &done.tag {
                *image = Some((range.start, range.end, dest_url.to_string()));
            }
            leaf.spans.push(Span {
                range: range.clone(),
                flags: LINK,
                markers: markers.clone(),
                url: true,
            });
        }
        _ => {}
    }
}

/// An emphasis-like span with `n` marker chars at each end.
pub(super) fn delimited(idx: &Index, range: Range<usize>, flags: Flags, n: usize) -> Span {
    let src = char_slice(idx, range.clone());
    let mut markers = Vec::new();
    if src.len() >= n * 2 {
        markers.push(range.start..range.start + n);
        markers.push(range.end - n..range.end);
    }
    Span {
        range,
        flags,
        markers,
        url: false,
    }
}
