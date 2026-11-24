use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::key::{ItemKey, Place, SectionKey, number};

#[derive(Clone, Debug)]
pub(super) struct Section {
    pub(super) key: SectionKey,
    pub(super) title: String,
    pub(super) start: usize,
    pub(super) title_range: Range<usize>,
    pub(super) end: usize,
}

impl Section {
    fn at(source: &str, heading: Range<usize>) -> Self {
        let start = source[..heading.start].rfind('\n').map_or(0, |i| i + 1);
        let first_end = source[start..]
            .find('\n')
            .map_or(source.len(), |i| start + i);
        let line = source[start..first_end].trim_end();
        let marks = line.trim_start().chars().take_while(|c| *c == '#').count();
        let text_start = if marks > 0 {
            let indent = line.len() - line.trim_start().len();
            let after = &line[indent + marks..];
            start + indent + marks + (after.len() - after.trim_start().len())
        } else {
            start + (line.len() - line.trim_start().len())
        };
        let text_end = (start + line.len()).max(text_start);
        Self {
            key: SectionKey::default(),
            title: source[text_start..text_end].to_owned(),
            start,
            title_range: text_start..text_end,
            end: source.len(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Entry {
    pub(super) key: ItemKey,
    pub(super) range: Range<usize>,
    pub(super) title: String,
    pub(super) details: String,
    pub(super) checked: Option<bool>,
    pub(super) section: Option<usize>,
    pub(super) editable: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Doc {
    pub(super) source: String,
    pub(super) sections: Vec<Section>,
    pub(super) entries: Vec<Entry>,
}

impl Doc {
    pub(super) fn parse(source: String) -> Self {
        let (sections, entries) = parse_parts(&source);
        Self {
            source,
            sections,
            entries,
        }
    }

    pub(super) fn find(&self, key: &ItemKey) -> Option<usize> {
        self.entries.iter().position(|entry| entry.key == *key)
    }

    /// The section index a place names: `None` if gone, `Some(None)` for the top region.
    pub(super) fn region(&self, place: &Place) -> Option<Option<usize>> {
        match place {
            Place::Top => Some(None),
            Place::Section(key) => self.section_index(key).map(Some),
        }
    }

    pub(super) fn section_index(&self, key: &SectionKey) -> Option<usize> {
        self.sections.iter().position(|section| section.key == *key)
    }

    /// The regions that get a group: the top when it has items or there are no sections.
    pub(super) fn regions(&self) -> Vec<Option<usize>> {
        let top = self.sections.is_empty() || self.entries.iter().any(|e| e.section.is_none());
        top.then_some(None)
            .into_iter()
            .chain((0..self.sections.len()).map(Some))
            .collect()
    }
}

pub(super) fn parse_parts(source: &str) -> (Vec<Section>, Vec<Entry>) {
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TASKLISTS | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    );
    let mut found = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    let mut list_depth = 0;
    let mut quote_depth = 0;
    let mut seen_title = false;
    let mut item: Option<Entry> = None;
    let mut paragraph: Option<Entry> = None;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) if list_depth == 0 && quote_depth == 0 => {
                if level == HeadingLevel::H1 && !seen_title {
                    seen_title = true;
                } else if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    sections.push(Section::at(source, range.clone()));
                }
            }
            Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote_depth -= 1,
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Item) if list_depth == 1 => {
                item = Some(Entry {
                    range: range.clone(),
                    section: sections.len().checked_sub(1),
                    editable: true,
                    ..Entry::default()
                })
            }
            Event::TaskListMarker(checked) if item.is_some() && list_depth == 1 => {
                if let Some(item) = &mut item {
                    item.checked = Some(checked);
                }
            }
            Event::End(TagEnd::Item) if list_depth == 1 => {
                if let Some(mut item) = item.take() {
                    item.range.end = range.end;
                    let raw = &source[item.range.clone()];
                    let (title, details, editable) = item_text(raw);
                    item.title = title;
                    item.details = details;
                    item.editable &= editable;
                    found.push(item);
                }
            }
            Event::Start(Tag::Paragraph) if list_depth == 0 => {
                paragraph = Some(Entry {
                    range: range.clone(),
                    section: sections.len().checked_sub(1),
                    editable: true,
                    ..Entry::default()
                })
            }
            Event::End(TagEnd::Paragraph) if list_depth == 0 => {
                if let Some(mut note) = paragraph.take() {
                    note.range.end = range.end;
                    let lines = source[note.range.clone()].trim().lines();
                    note.title = lines.map(str::trim).collect::<Vec<_>>().join(" ");
                    found.push(note);
                }
            }
            Event::Start(Tag::CodeBlock(_)) | Event::Start(Tag::Table(_)) => {
                if let Some(item) = &mut item {
                    item.editable = false;
                }
            }
            _ => {}
        }
    }
    for index in 0..sections.len() {
        let end = sections
            .get(index + 1)
            .map_or(source.len(), |next| next.start);
        sections[index].end = end;
    }
    let keys = number(sections.iter().map(|section| section.title.clone()));
    for (section, key) in sections.iter_mut().zip(keys) {
        section.key = SectionKey::from(key);
    }
    let keys = number(
        found
            .iter()
            .map(|entry| item_identity(&source[entry.range.clone()])),
    );
    for (entry, key) in found.iter_mut().zip(keys) {
        entry.key = ItemKey::from(key);
    }
    (sections, found)
}

fn item_identity(raw: &str) -> String {
    let raw = raw.trim();
    match raw
        .strip_prefix("- [x] ")
        .or_else(|| raw.strip_prefix("- [X] "))
    {
        Some(rest) => format!("- [ ] {rest}"),
        None => raw.to_owned(),
    }
}

fn item_text(raw: &str) -> (String, String, bool) {
    let mut lines = raw.trim_end().lines();
    let first = lines.next().unwrap_or("");
    let mut title = first
        .strip_prefix("- [ ] ")
        .or_else(|| first.strip_prefix("- [x] "))
        .or_else(|| first.strip_prefix("- [X] "))
        .or_else(|| first.strip_prefix("- "))
        .unwrap_or(first)
        .trim()
        .to_owned();
    let mut editable = true;
    let mut bullets: Vec<String> = Vec::new();
    for line in lines {
        let body = line.trim();
        if body.is_empty() {
            continue;
        }
        let marked = ["- ", "* ", "+ "]
            .iter()
            .find_map(|marker| body.strip_prefix(marker));
        let Some(text) = marked else {
            let last = bullets.last_mut().unwrap_or(&mut title);
            last.push(' ');
            last.push_str(body);
            continue;
        };
        if body.starts_with("- [") || body.starts_with("* [") {
            editable = false;
        }
        let spaces = line.chars().take_while(|c| *c == ' ').count();
        bullets.push(format!(
            "{}{}",
            " ".repeat(spaces.saturating_sub(2)),
            text.trim()
        ));
    }
    (title, bullets.join("\n"), editable)
}

pub(super) fn safe_append_boundary(source: &str) -> bool {
    if source.lines().next() == Some("---")
        && !source
            .lines()
            .skip(1)
            .any(|line| line == "---" || line == "...")
    {
        return false;
    }
    let mut fence: Option<(char, usize)> = None;
    for line in source.lines() {
        let line = line.trim_start();
        let Some(marker) = line.chars().next().filter(|c| *c == '`' || *c == '~') else {
            continue;
        };
        let count = line.chars().take_while(|c| *c == marker).count();
        if count < 3 {
            continue;
        }
        match fence {
            None => fence = Some((marker, count)),
            Some((open, size))
                if marker == open && count >= size && line[count..].trim().is_empty() =>
            {
                fence = None
            }
            _ => {}
        }
    }
    fence.is_none()
}
