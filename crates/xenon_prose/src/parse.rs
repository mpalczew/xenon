//! pulldown-cmark events to [`Doc`]. Byte offsets from the parser become char
//! offsets here, once, so no caller has to convert.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::conv::Conv;
use crate::{Checkbox, CodeRef, Doc, ImageRef, Kind, LinkRef, Markup, Row, Span};

/// A link or image whose text extent is still being learned.
struct Frame {
    image: bool,
    url: String,
    /// Smallest start and largest end of the events inside.
    extent: Option<(usize, usize)>,
}

struct Walk<'a> {
    text: &'a str,
    conv: Conv<'a>,
    doc: Doc,
    stack: Vec<Frame>,
    list_depth: usize,
    /// Per open item: the first-line char range and the open tight text block.
    items: Vec<(Range<usize>, Option<usize>)>,
    in_leaf: bool,
}

pub(crate) fn parse(text: &str) -> Doc {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut walk = Walk {
        text,
        conv: Conv::new(text),
        doc: Doc::default(),
        stack: Vec::new(),
        list_depth: 0,
        items: Vec::new(),
        in_leaf: false,
    };
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        walk.event(event, range);
    }
    walk.finish()
}

impl Walk<'_> {
    fn span(&mut self, bytes: Range<usize>, kind: Kind) {
        if bytes.start < bytes.end {
            self.doc.spans.push(Span {
                range: self.conv.range(&bytes),
                kind,
            });
        }
    }

    /// A block-level span: it starts at the line start, so paragraph tags
    /// (margins, indent) see a tagged first character even when the block is
    /// nested inside a list item or quote.
    fn span_lines(&mut self, bytes: Range<usize>, kind: Kind) {
        let start = self.conv.line_start(bytes.start);
        self.span(start..bytes.end, kind);
    }

    fn mark(&mut self, bytes: Range<usize>) {
        if bytes.start < bytes.end {
            self.doc.markup.push(Markup {
                range: self.conv.range(&bytes),
                block: None,
            });
        }
    }

    /// A leaf block covering whole lines of `bytes`.
    fn block(&mut self, bytes: &Range<usize>) -> usize {
        let t = self.conv.trim_end(bytes);
        let start = self.conv.line_start(t.start);
        let end = self.conv.line_end(t.end);
        self.doc.blocks.push(self.conv.at(start)..self.conv.at(end));
        self.doc.blocks.len() - 1
    }

    /// Tight list items have text with no paragraph; give it a block.
    fn open_item_text(&mut self) {
        if self.in_leaf {
            return;
        }
        let Some((line, open)) = self.items.last().cloned() else {
            return;
        };
        if open.is_none() {
            let start = line.start;
            self.doc.blocks.push(start..line.end.max(start));
            let ix = self.doc.blocks.len() - 1;
            if let Some(top) = self.items.last_mut() {
                top.1 = Some(ix);
            }
        }
    }

    fn close_item_text(&mut self, end_byte: usize) {
        let end = self.conv.at(end_byte);
        if let Some((_, open)) = self.items.last_mut()
            && let Some(ix) = open.take()
        {
            let b = &mut self.doc.blocks[ix];
            b.end = end.max(b.start);
        }
    }

    fn touch(&mut self, r: &Range<usize>) {
        for f in &mut self.stack {
            f.extent = Some(match f.extent {
                Some((a, b)) => (a.min(r.start), b.max(r.end)),
                None => (r.start, r.end),
            });
        }
    }

    fn event(&mut self, event: Event, r: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag, r),
            Event::End(end) => self.end(end, &r),
            Event::Text(_) | Event::SoftBreak | Event::HardBreak | Event::InlineHtml(_) => {
                self.open_item_text();
                self.touch(&r);
            }
            Event::Code(_) => {
                self.open_item_text();
                self.touch(&r);
                let ticks = self.text[r.clone()]
                    .bytes()
                    .take_while(|b| *b == b'`')
                    .count();
                self.span(r.clone(), Kind::Code);
                self.mark(r.start..r.start + ticks);
                self.mark(r.end.saturating_sub(ticks).max(r.start + ticks)..r.end);
            }
            Event::Rule => {
                let t = self.conv.trim_end(&r);
                self.span_lines(t, Kind::Rule);
            }
            Event::TaskListMarker(checked) => {
                let line = self.items.last().map(|i| i.0.clone()).unwrap_or_default();
                let range = self.conv.range(&r);
                // The checkbox stands in for the bullet of its item.
                self.doc.bullets.pop();
                self.doc.spans.push(Span {
                    range: range.clone(),
                    kind: Kind::Checkbox(checked),
                });
                if checked {
                    self.doc.spans.push(Span {
                        range: line.clone(),
                        kind: Kind::Done,
                    });
                }
                self.doc.checkboxes.push(Checkbox {
                    range,
                    line,
                    checked,
                });
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn start(&mut self, tag: Tag, r: Range<usize>) {
        if !matches!(tag, Tag::Item | Tag::List(_)) {
            self.touch(&r);
        }
        match tag {
            Tag::Heading { level, .. } => {
                self.in_leaf = true;
                self.block(&r);
                let t = self.conv.trim_end(&r);
                self.span_lines(t.clone(), Kind::Heading(level as u8));
                let src = &self.text[t.clone()];
                if src.starts_with('#') {
                    let hashes = src.bytes().take_while(|b| *b == b'#').count();
                    let gap = src[hashes..]
                        .bytes()
                        .take_while(|b| *b == b' ' || *b == b'\t')
                        .count();
                    self.mark(t.start..t.start + hashes + gap);
                } else if let Some(nl) = src.rfind('\n') {
                    // Setext: the underline is the last line.
                    self.mark(t.start + nl..t.end);
                }
            }
            Tag::Paragraph => {
                self.in_leaf = true;
                self.block(&r);
            }
            Tag::CodeBlock(kind) => {
                self.in_leaf = true;
                self.block(&r);
                let t = self.conv.trim_end(&r);
                self.span_lines(t.clone(), Kind::CodeBlock);
                if let CodeBlockKind::Fenced(info) = kind {
                    let body = self.fences(&t);
                    let lang = info.split_whitespace().next().unwrap_or_default();
                    if let (Some(body), false) = (body, lang.is_empty()) {
                        self.doc.code.push(CodeRef {
                            lang: lang.to_string(),
                            body: self.conv.range(&body),
                        });
                    }
                }
            }
            Tag::TableHead | Tag::TableRow => {
                let head = matches!(tag, Tag::TableHead);
                let (a, b) = (self.conv.line_start(r.start), self.conv.line_end(r.start));
                let pipes = (a..b)
                    .filter(|&i| {
                        self.text.as_bytes()[i] == b'|'
                            && (i == a || self.text.as_bytes()[i - 1] != b'\\')
                    })
                    .map(|i| self.conv.at(i))
                    .collect();
                self.doc.rows.push(Row {
                    line: self.conv.range(&(a..b)),
                    pipes,
                    top: head,
                });
                if head {
                    // The `---|---` line under the header hides with the markup.
                    let next = (b + 1).min(self.text.len());
                    self.mark(next..(self.conv.line_end(next) + 1).min(self.text.len()));
                }
            }
            Tag::Table(_) | Tag::HtmlBlock => {
                self.in_leaf = true;
                self.block(&r);
                let t = self.conv.trim_end(&r);
                self.span_lines(t, Kind::Verbatim);
            }
            Tag::MetadataBlock(_) => {
                self.in_leaf = true;
                self.block(&r);
                let t = self.conv.trim_end(&r);
                self.span_lines(t, Kind::Meta);
            }
            Tag::BlockQuote(_) => {
                let t = self.conv.trim_end(&r);
                self.span_lines(t.clone(), Kind::Quote);
                self.quote_marks(&t);
            }
            Tag::List(_) => {
                let at = self.conv.line_start(r.start);
                // Text before a nested list ends where the list begins.
                self.close_item_text(at.saturating_sub(1));
                self.list_depth += 1;
            }
            Tag::Item => {
                let depth = self.list_depth.saturating_sub(1).min(3) as u8;
                let first = self.conv.line_start(r.start)..self.conv.line_end(r.start);
                let line = self.conv.range(&first);
                self.span(first.clone(), Kind::ListItem(depth));
                let first = r.start..first.end;
                let src = &self.text[first.clone()];
                let digits = src.bytes().take_while(u8::is_ascii_digit).count();
                let len = if digits > 0 {
                    (src.len() > digits).then_some(digits + 1)
                } else {
                    Some(1)
                };
                if let Some(len) = len {
                    self.span(r.start..r.start + len, Kind::ListMarker);
                    if digits == 0 {
                        self.doc.bullets.push(Markup {
                            range: self.conv.range(&(r.start..r.start + 1)),
                            block: None,
                        });
                    }
                }
                self.items.push((line, None));
            }
            Tag::Emphasis => self.delimited(&r, 1, Kind::Italic),
            Tag::Strong => self.delimited(&r, 2, Kind::Bold),
            Tag::Strikethrough => {
                let n = self.text[r.clone()]
                    .bytes()
                    .take_while(|b| *b == b'~')
                    .count()
                    .clamp(1, 2);
                self.delimited(&r, n, Kind::Strike);
            }
            Tag::Link { dest_url, .. } => self.stack.push(Frame {
                image: false,
                url: dest_url.to_string(),
                extent: None,
            }),
            Tag::Image { dest_url, .. } => self.stack.push(Frame {
                image: true,
                url: dest_url.to_string(),
                extent: None,
            }),
            _ => {}
        }
    }

    fn delimited(&mut self, r: &Range<usize>, n: usize, kind: Kind) {
        self.open_item_text();
        self.span(r.clone(), kind);
        self.mark(r.start..(r.start + n).min(r.end));
        self.mark(r.end.saturating_sub(n).max(r.start + n)..r.end);
    }

    /// Mark the fence lines; returns the bytes between them.
    fn fences(&mut self, t: &Range<usize>) -> Option<Range<usize>> {
        let first_end = self.conv.line_end(t.start);
        let fence = |s: &str| {
            let s = s.trim_start();
            s.starts_with("```") || s.starts_with("~~~")
        };
        if !fence(&self.text[t.start..first_end]) {
            return None;
        }
        self.mark(t.start..(first_end + 1).min(self.text.len()));
        let body = (first_end + 1).min(t.end);
        let last = self.conv.line_start(t.end);
        if last > first_end && fence(&self.text[last..t.end]) {
            self.mark(last..(self.conv.line_end(last) + 1).min(self.text.len()));
            return Some(body..last.max(body));
        }
        Some(body..t.end)
    }

    fn quote_marks(&mut self, t: &Range<usize>) {
        let mut ls = self.conv.line_start(t.start);
        while ls < t.end {
            let bytes = self.text.as_bytes();
            let mut i = ls;
            loop {
                let mut j = i;
                while j < ls + 3 + (i - ls) && bytes.get(j) == Some(&b' ') {
                    j += 1;
                }
                if bytes.get(j) == Some(&b'>') {
                    i = j + 1;
                    if bytes.get(i) == Some(&b' ') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if i > ls {
                self.mark(ls..i);
            }
            ls = self.conv.line_end(ls) + 1;
        }
    }

    fn end(&mut self, end: TagEnd, r: &Range<usize>) {
        match end {
            TagEnd::Heading(_)
            | TagEnd::Paragraph
            | TagEnd::CodeBlock
            | TagEnd::Table
            | TagEnd::HtmlBlock
            | TagEnd::MetadataBlock(_) => self.in_leaf = false,
            TagEnd::List(_) => self.list_depth = self.list_depth.saturating_sub(1),
            TagEnd::Item => {
                let t = self.conv.trim_end(r);
                self.close_item_text(t.end);
                self.items.pop();
            }
            TagEnd::Link | TagEnd::Image => self.close_frame(r),
            _ => {}
        }
    }

    fn close_frame(&mut self, r: &Range<usize>) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let (a, b) = frame
            .extent
            .filter(|(a, b)| r.start < *a && *b <= r.end && a < b)
            .unwrap_or((r.start + 1, r.start + 1));
        self.mark(r.start..a);
        self.mark(b..r.end);
        if frame.image {
            self.span(a..b, Kind::ImageAlt);
            let line_end = self.conv.at(self.conv.line_end(r.end));
            self.doc.images.push(ImageRef {
                range: self.conv.range(r),
                line_end,
                url: frame.url,
            });
        } else {
            self.span(a..b, Kind::Link);
            self.doc.links.push(LinkRef {
                range: self.conv.range(&(a..b)),
                url: frame.url,
            });
        }
    }

    fn finish(mut self) -> Doc {
        let doc = &mut self.doc;
        doc.spans.sort_by_key(|s| s.range.start);
        doc.markup.sort_by_key(|m| m.range.start);
        doc.markup.dedup_by(|b, a| a.range.end > b.range.start);
        doc.links.sort_by_key(|l| l.range.start);
        doc.blocks.sort_by_key(|b| b.start);
        let lookup = |pos: usize, blocks: &[Range<usize>]| {
            let ix = blocks.partition_point(|b| b.start <= pos).checked_sub(1)?;
            (pos <= blocks[ix].end).then_some(ix)
        };
        doc.bullets.sort_by_key(|m| m.range.start);
        for m in doc.markup.iter_mut().chain(&mut doc.bullets) {
            m.block = lookup(m.range.start, &doc.blocks);
        }
        self.doc
    }
}
