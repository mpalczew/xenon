//! What a worklist list view needs from the file: which rows are visible and
//! where the cursor sits, the checkbox flip, section counts and inline Markdown.

use std::collections::BTreeSet;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::SourceEdit;
use super::entries::Entry;
use super::sections::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Item(usize),
    Header(usize),
}

/// Rows in reading order: items above the first section, then each header
/// followed by its items unless the section is folded.
pub fn visible(sections: &[Section], entries: &[Entry], folded: &BTreeSet<String>) -> Vec<Cursor> {
    let items = |section: Option<usize>| {
        entries
            .iter()
            .enumerate()
            .filter(move |(_, entry)| entry.section == section)
            .map(|(index, _)| Cursor::Item(index))
    };
    let mut rows: Vec<Cursor> = items(None).collect();
    for (index, section) in sections.iter().enumerate() {
        rows.push(Cursor::Header(index));
        if !folded.contains(&section.title) {
            rows.extend(items(Some(index)));
        }
    }
    rows
}

/// The nearest valid row when edits, folds, or external changes moved the cursor.
pub fn settle(rows: &[Cursor], cursor: Cursor, entries: &[Entry]) -> Cursor {
    if rows.contains(&cursor) {
        return cursor;
    }
    if let Cursor::Item(index) = cursor
        && let Some(section) = entries.get(index).and_then(|entry| entry.section)
        && rows.contains(&Cursor::Header(section))
    {
        return Cursor::Header(section);
    }
    let same_kind = |row: &&Cursor| std::mem::discriminant(*row) == std::mem::discriminant(&cursor);
    rows.iter()
        .rev()
        .find(same_kind)
        .or(rows.last())
        .copied()
        .unwrap_or(Cursor::Item(0))
}

/// "2 open", "all done", or nothing for an empty section.
pub fn count_label(open: usize, items: usize) -> Option<String> {
    match (open, items) {
        (0, 0) => None,
        (0, _) => Some("all done".into()),
        (open, _) => Some(format!("{open} open")),
    }
}

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
