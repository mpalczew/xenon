use anyhow::{Context, Result, ensure};
use unicode_segmentation::UnicodeSegmentation;

use super::doc::{Entry, Target, parse_parts};
use super::edit::{SourceEdit, append_item, newline_of, removal_range, trimmed};

pub(super) const TITLE_LIMIT: usize = 80;

pub(super) fn title_length(title: &str) -> usize {
    title.graphemes(true).count()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OutlinePoint {
    text: String,
    depth: u8,
}

fn points_from_details(details: &str) -> Vec<OutlinePoint> {
    details
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let spaces = line
                .chars()
                .take_while(|character| *character == ' ')
                .count();
            let body = line[spaces..].trim();
            let text = body.strip_prefix("- ").unwrap_or(body).trim().to_owned();
            if text.is_empty() {
                return None;
            }
            Some(OutlinePoint {
                text,
                depth: u8::try_from(spaces / 2).unwrap_or(u8::MAX),
            })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WorkItem {
    title: String,
    details: Vec<OutlinePoint>,
    checked: Option<bool>,
}

impl WorkItem {
    pub(super) fn new(title: &str, details: &str, checked: Option<bool>) -> Result<Self> {
        let title = title.trim();
        ensure!(!title.is_empty(), "Enter a title");
        ensure!(!title.contains(['\r', '\n']), "Title must be one line");
        let details = points_from_details(details);
        Ok(Self {
            title: title.to_owned(),
            details,
            checked,
        })
    }

    pub(super) fn markdown(&self, newline: &str) -> String {
        let marker = match self.checked {
            None => "- ",
            Some(true) => "- [x] ",
            Some(false) => "- [ ] ",
        };
        let mut out = format!("{marker}{}", self.title);
        for detail in &self.details {
            let indent = 2 + usize::from(detail.depth) * 2;
            out.push_str(newline);
            out.push_str(&" ".repeat(indent));
            out.push_str("- ");
            out.push_str(&detail.text);
        }
        out
    }
}

pub(super) enum DraftEdit<'a> {
    Save(&'a WorkItem),
    Remove,
}

pub(super) struct DraftWrite {
    pub(super) updated: String,
    item_start: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Draft {
    saved: Option<Saved>,
    target: Target,
}

#[derive(Clone, Debug)]
struct Saved {
    start: usize,
    raw: String,
}

impl Draft {
    pub(super) fn existing(source: &str, entry: &Entry) -> Self {
        Self {
            saved: Some(Saved {
                start: entry.range.start,
                raw: trimmed(&source[entry.range.clone()]).to_owned(),
            }),
            target: Target::Top,
        }
    }

    pub(super) fn set_target(&mut self, target: Target) {
        self.target = target;
    }

    pub(super) fn is_saved(&self) -> bool {
        self.saved.is_some()
    }

    pub(super) fn title(&self) -> Option<&str> {
        let line = self.saved.as_ref()?.raw.lines().next()?;
        let title = ["- [x] ", "- [X] ", "- [ ] ", "- "]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
            .unwrap_or(line)
            .trim();
        (!title.is_empty()).then_some(title)
    }

    pub(super) fn prepare(&self, source: &str, edit: DraftEdit<'_>) -> Result<Option<DraftWrite>> {
        match edit {
            DraftEdit::Save(item) => self.save(source, item),
            DraftEdit::Remove => self.remove(source),
        }
    }

    fn save(&self, source: &str, item: &WorkItem) -> Result<Option<DraftWrite>> {
        let Some(start) = self.locate(source)? else {
            let insertion = append_item(source, &self.target, item)?;
            let edit = SourceEdit {
                range: insertion.range,
                text: insertion.text,
            };
            return Ok(Some(DraftWrite {
                updated: edit.apply(source),
                item_start: Some(insertion.block_start),
            }));
        };
        let text = item.markdown(newline_of(source));
        let end = start + self.saved.as_ref().map_or(0, |saved| saved.raw.len());
        if source[start..end] == text {
            return Ok(None);
        }
        let edit = SourceEdit {
            range: start..end,
            text,
        };
        Ok(Some(DraftWrite {
            updated: edit.apply(source),
            item_start: Some(start),
        }))
    }

    fn remove(&self, source: &str) -> Result<Option<DraftWrite>> {
        let Some(start) = self.locate(source)? else {
            return Ok(None);
        };
        let (_, entries) = parse_parts(source);
        let range = entries
            .into_iter()
            .find(|entry| entry.range.start == start)
            .context("Worklist item is gone")?
            .range;
        let edit = SourceEdit {
            range: removal_range(source, range),
            text: String::new(),
        };
        Ok(Some(DraftWrite {
            updated: edit.apply(source),
            item_start: None,
        }))
    }

    pub(super) fn commit(&mut self, write: DraftWrite) {
        let Some(start) = write.item_start else {
            self.saved = None;
            return;
        };
        let (_, entries) = parse_parts(&write.updated);
        let Some(range) = entries
            .into_iter()
            .find(|entry| entry.range.start == start)
            .map(|entry| entry.range)
        else {
            return;
        };
        self.saved = Some(Saved {
            start: range.start,
            raw: trimmed(&write.updated[range]).to_owned(),
        });
    }

    fn locate(&self, source: &str) -> Result<Option<usize>> {
        let Some(saved) = &self.saved else {
            return Ok(None);
        };
        if source.get(saved.start..saved.start + saved.raw.len()) == Some(saved.raw.as_str()) {
            return Ok(Some(saved.start));
        }
        let (_, entries) = parse_parts(source);
        entries
            .into_iter()
            .find(|entry| trimmed(&source[entry.range.clone()]) == saved.raw)
            .map(|entry| Some(entry.range.start))
            .context("This item changed elsewhere; reopen it to keep editing")
    }
}
