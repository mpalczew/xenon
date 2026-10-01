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

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(line: &str) -> (String, Vec<(&'static str, String)>) {
        let marked = marked(line);
        let text = marked.text.to_string();
        let named = marked
            .marks
            .iter()
            .map(|(range, marks)| {
                let name = match marks {
                    Marks { code: true, .. } => "code",
                    Marks {
                        bold: true,
                        italic: true,
                        ..
                    } => "bold italic",
                    Marks { bold: true, .. } => "bold",
                    Marks { italic: true, .. } => "italic",
                    Marks { strike: true, .. } => "strike",
                    Marks { link: true, .. } => "link",
                    _ => "none",
                };
                (name, text[range.clone()].to_owned())
            })
            .collect();
        (text, named)
    }

    #[test]
    fn bold_and_code_lose_their_delimiters() {
        let (text, marks) = spans("**Ship 2.19**: fix `/v1/me` errors");
        assert_eq!(text, "Ship 2.19: fix /v1/me errors");
        assert_eq!(
            marks,
            [("bold", "Ship 2.19".into()), ("code", "/v1/me".into())]
        );
    }

    #[test]
    fn nested_emphasis_and_links() {
        let (text, marks) = spans("**a *b* c** ~~old~~ [docs](https://x.dev)");
        assert_eq!(text, "a b c old docs");
        assert_eq!(
            marks,
            [
                ("bold", "a ".into()),
                ("bold italic", "b".into()),
                ("bold", " c".into()),
                ("strike", "old".into()),
                ("link", "docs".into()),
            ]
        );
    }

    #[test]
    fn block_shaped_lines_stay_literal() {
        assert_eq!(spans("1. first step").0, "1. first step");
        assert_eq!(spans("# not a heading").0, "# not a heading");
        assert_eq!(spans("> quoted").0, "> quoted");
    }

    #[test]
    fn plain_and_unclosed_text_is_unchanged() {
        assert_eq!(spans("2 * 3 = 6, **open").0, "2 * 3 = 6, **open");
        assert!(spans("plain words").1.is_empty());
    }
}
