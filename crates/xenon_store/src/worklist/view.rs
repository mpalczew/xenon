use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::SourceEdit;
use super::entries::Entry;

/// Flips the check box of `entry` in `source`; None for a note or a row that cannot be edited.
pub fn toggle(source: &str, entry: &Entry) -> Option<SourceEdit> {
    let checked = entry.checked?;
    if !entry.editable {
        return None;
    }
    let raw = &source[entry.range.clone()];
    let mark = raw.find(if checked { "[x]" } else { "[ ]" })?;
    let at = entry.range.start + mark + 1;
    Some(SourceEdit {
        range: at..at + 1,
        text: if checked { " " } else { "x" }.into(),
    })
}

/// One worklist line as Pango markup: bold, italic, strikethrough, links and
/// code. Lines Markdown would read as a block (`1. step`, `# tag`) stay literal.
pub fn inline_markup(line: &str) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut events = Parser::new_ext(line, Options::ENABLE_STRIKETHROUGH);
    if !matches!(events.next(), Some(Event::Start(Tag::Paragraph))) {
        return escape(line);
    }
    let mut out = String::new();
    for event in events {
        match event {
            Event::Start(Tag::Strong) => out.push_str("<b>"),
            Event::End(TagEnd::Strong) => out.push_str("</b>"),
            Event::Start(Tag::Emphasis) => out.push_str("<i>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</i>"),
            Event::Start(Tag::Strikethrough) => out.push_str("<s>"),
            Event::End(TagEnd::Strikethrough) => out.push_str("</s>"),
            Event::Start(Tag::Link { .. }) => out.push_str("<u>"),
            Event::End(TagEnd::Link) => out.push_str("</u>"),
            Event::Text(piece) | Event::Html(piece) | Event::InlineHtml(piece) => {
                out.push_str(&escape(&piece))
            }
            Event::Code(piece) => {
                out.push_str("<tt>");
                out.push_str(&escape(&piece));
                out.push_str("</tt>");
            }
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out
}
