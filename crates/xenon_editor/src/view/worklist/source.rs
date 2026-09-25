use gpui::Entity;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;
use xenon_design_system::TextInputView;

#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub(super) range: Range<usize>,
    pub(super) text: String,
    pub(super) checked: Option<bool>,
    pub(super) section: usize,
    pub(super) editable: bool,
}

pub(in crate::view) struct ItemEdit {
    pub(super) entry: Entry,
    pub input: Entity<TextInputView>,
}

pub(super) fn entries(source: &str) -> Vec<Entry> {
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
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
                if list_depth > 1
                    && let Some(item) = &mut item
                {
                    item.editable = false;
                }
            }
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Item) if list_depth == 1 => {
                item = Some(Entry {
                    range: range.clone(),
                    text: String::new(),
                    checked: None,
                    section,
                    editable: true,
                })
            }
            Event::TaskListMarker(checked) if item.is_some() => {
                if let Some(item) = &mut item {
                    item.checked = Some(checked);
                }
            }
            Event::End(TagEnd::Item) if list_depth == 1 => {
                if let Some(mut item) = item.take() {
                    item.range.end = range.end;
                    if item.checked.is_some() {
                        let raw = &source[item.range.clone()];
                        item.text = task_text(raw);
                        found.push(item);
                    }
                }
            }
            Event::Start(Tag::Paragraph) if list_depth == 0 => {
                paragraph = Some(Entry {
                    range: range.clone(),
                    text: String::new(),
                    checked: None,
                    section,
                    editable: true,
                })
            }
            Event::End(TagEnd::Paragraph) if list_depth == 0 => {
                if let Some(mut note) = paragraph.take() {
                    note.range.end = range.end;
                    note.text = source[note.range.clone()].trim().to_string();
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

fn task_text(raw: &str) -> String {
    let mut lines = raw.trim_end().lines();
    let first = lines.next().unwrap_or("");
    let first = first.find("] ").map_or(first, |i| &first[i + 2..]);
    std::iter::once(first)
        .chain(lines.map(|line| line.strip_prefix("  ").unwrap_or(line)))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub(super) fn replacement(entry: &Entry, text: &str, newline: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    match entry.checked {
        None => normalized.replace('\n', newline),
        Some(done) => {
            let marker = if done { "- [x] " } else { "- [ ] " };
            let mut lines = normalized.lines();
            let mut out = format!("{marker}{}", lines.next().unwrap_or(""));
            for line in lines {
                out.push_str(newline);
                out.push_str("  ");
                out.push_str(line);
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_tasks_and_notes_without_treating_fences_as_items() {
        let source = "# Worklist\n\n- [ ] First\n  detail\n\nA note.\n\n```md\n- [ ] not a task\n```\n\n- [x] Done\n";
        let rows = entries(source);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text, "First\ndetail");
        assert_eq!(rows[1].text, "A note.");
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
    fn item_replacement_keeps_task_shape_and_crlf() {
        let source = "- [x] First\r\n  Detail\r\n\r\nNext note.\r\n";
        let rows = entries(source);
        assert_eq!(rows.len(), 2);
        let old = &source[rows[0].range.clone()];
        let suffix = &old[old.trim_end_matches(['\r', '\n']).len()..];
        let updated = source.replace(
            old,
            &(replacement(&rows[0], "Revised\nMore", "\r\n") + suffix),
        );
        assert_eq!(updated, "- [x] Revised\r\n  More\r\n\r\nNext note.\r\n");
    }
}
