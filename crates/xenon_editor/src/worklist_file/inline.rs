//! Inline Markdown in one worklist line, as display text plus emphasis marks.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use xenon_design_system::{MarkedText, Marks};

/// Lines that Markdown would read as a block (`1. step`, `# tag`) stay literal.
pub(crate) fn marked(line: &str) -> MarkedText {
    let mut events = Parser::new_ext(line, Options::ENABLE_STRIKETHROUGH);
    if !matches!(events.next(), Some(Event::Start(Tag::Paragraph))) {
        return line.into();
    }
    let mut out = Builder::default();
    for event in events {
        out.event(event);
    }
    MarkedText {
        text: out.text.into(),
        marks: out.marks,
    }
}

/// Open-tag depths, so nested `***x***` and `**a *b* c**` close correctly.
#[derive(Default)]
struct Builder {
    text: String,
    marks: Vec<(std::ops::Range<usize>, Marks)>,
    bold: u8,
    italic: u8,
    strike: u8,
    link: u8,
}

impl Builder {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.depth(&tag_end(tag), 1),
            Event::End(tag) => self.depth(&Some(tag), -1),
            Event::Text(piece) | Event::Html(piece) | Event::InlineHtml(piece) => {
                self.push(&piece, false)
            }
            Event::Code(piece) => self.push(&piece, true),
            Event::SoftBreak | Event::HardBreak => self.push(" ", false),
            _ => {}
        }
    }

    fn depth(&mut self, tag: &Option<TagEnd>, delta: i8) {
        let counter = match tag {
            Some(TagEnd::Strong) => &mut self.bold,
            Some(TagEnd::Emphasis) => &mut self.italic,
            Some(TagEnd::Strikethrough) => &mut self.strike,
            Some(TagEnd::Link) => &mut self.link,
            _ => return,
        };
        *counter = counter.saturating_add_signed(delta);
    }

    fn push(&mut self, piece: &str, code: bool) {
        let start = self.text.len();
        self.text.push_str(piece);
        let marks = Marks {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            link: self.link > 0,
            code,
        };
        if marks != Marks::default() {
            self.marks.push((start..self.text.len(), marks));
        }
    }
}

fn tag_end(tag: Tag) -> Option<TagEnd> {
    match tag {
        Tag::Strong => Some(TagEnd::Strong),
        Tag::Emphasis => Some(TagEnd::Emphasis),
        Tag::Strikethrough => Some(TagEnd::Strikethrough),
        Tag::Link { .. } => Some(TagEnd::Link),
        _ => None,
    }
}
