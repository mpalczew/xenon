//! Top-level worklist items located in the Markdown source.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) range: Range<usize>,
    pub(crate) title: String,
    pub(crate) details: String,
    pub(crate) checked: Option<bool>,
    #[allow(dead_code)]
    pub(crate) section: usize,
    pub(crate) editable: bool,
}

pub(crate) fn entries(source: &str) -> Vec<Entry> {
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TASKLISTS | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    );
    let mut found = Vec::new();
    let mut list_depth = 0;
    let mut section = 0;
    let mut item: Option<Entry> = None;
    let mut paragraph: Option<Entry> = None;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => section += 1,
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Item) if list_depth == 1 => {
                item = Some(Entry {
                    range: range.clone(),
                    title: String::new(),
                    details: String::new(),
                    checked: None,
                    section,
                    editable: true,
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
                    title: String::new(),
                    details: String::new(),
                    checked: None,
                    section,
                    editable: true,
                })
            }
            Event::End(TagEnd::Paragraph) if list_depth == 0 => {
                if let Some(mut note) = paragraph.take() {
                    note.range.end = range.end;
                    let mut lines = source[note.range.clone()].trim().lines();
                    note.title = lines.next().unwrap_or("").to_owned();
                    note.details = lines.collect::<Vec<_>>().join("\n");
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
    found
}

fn item_text(raw: &str) -> (String, String, bool) {
    let mut lines = raw.trim_end().lines();
    let first = lines.next().unwrap_or("");
    let title = first
        .strip_prefix("- [ ] ")
        .or_else(|| first.strip_prefix("- [x] "))
        .or_else(|| first.strip_prefix("- [X] "))
        .or_else(|| first.strip_prefix("- "))
        .unwrap_or(first)
        .trim()
        .to_owned();
    let mut editable = true;
    let details = lines
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let spaces = line
                .chars()
                .take_while(|character| *character == ' ')
                .count();
            let body = line.trim_start();
            if body.starts_with("- [") || body.starts_with("* [") {
                editable = false;
            }
            let text = body
                .strip_prefix("- ")
                .or_else(|| body.strip_prefix("* "))
                .unwrap_or(body)
                .trim();
            Some(format!("{}{text}", " ".repeat(spaces.saturating_sub(2))))
        })
        .collect::<Vec<_>>()
        .join("\n");
    (title, details, editable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_tasks_and_notes_without_treating_fences_as_items() {
        let source = "# Worklist\n\n- [ ] First\n  detail\n\nA note.\n\n```md\n- [ ] not a task\n```\n\n- [x] Done\n";
        let rows = entries(source);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].title, "First");
        assert_eq!(rows[0].details, "detail");
        assert_eq!(rows[1].title, "A note.");
        assert_eq!(rows[2].checked, Some(true));
        assert_eq!(&source[rows[0].range.clone()], "- [ ] First\n  detail\n\n");
        assert_eq!(&source[rows[1].range.clone()], "A note.\n");
    }

    #[test]
    fn nested_tasks_remain_visible_but_require_markdown_editing() {
        let source = "# One\n\n- [ ] Parent\n  - [ ] Child\n\n# Two\n\n- [ ] Other\n";
        let rows = entries(source);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].editable);
        assert!(rows[1].editable);
        assert_ne!(rows[0].section, rows[1].section);
    }

    #[test]
    fn title_and_bullets_are_separate_for_tasks_and_notes() {
        let rows = entries(
            "# Worklist\n\n- [ ] Task title\n  - First point\n  - Second point\n\n- Note title\n  - Context\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Task title");
        assert_eq!(rows[0].details, "First point\nSecond point");
        assert!(rows[0].editable);
        assert_eq!(rows[1].title, "Note title");
        assert_eq!(rows[1].details, "Context");
        assert!(rows[1].editable);
    }
}
