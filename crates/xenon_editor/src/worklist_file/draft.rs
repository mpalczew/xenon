//! One item carried across autosaves: appended on the first save, rewritten in place after.

use super::entries::entries;
use super::{Target, WorkItem, append_item};
use anyhow::{Context, Result};
use std::ops::Range;

/// A source edit: `range` becomes `text`.
#[derive(Clone, Debug)]
pub struct Change {
    pub range: Range<usize>,
    pub text: String,
    /// Where the item starts after the edit; None once it is removed.
    item_start: Option<usize>,
}

impl Change {
    pub fn apply(&self, source: &str) -> String {
        let mut out = source.to_owned();
        out.replace_range(self.range.clone(), &self.text);
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct ItemDraft {
    saved: Option<Saved>,
    checked: bool,
    /// Where the first save files the item.
    target: Target,
}

/// The item's Markdown as last written, without trailing blank lines.
#[derive(Clone, Debug)]
struct Saved {
    start: usize,
    raw: String,
}

impl ItemDraft {
    pub(crate) fn existing(source: &str, range: Range<usize>, checked: bool) -> Self {
        Self {
            saved: Some(Saved {
                start: range.start,
                raw: trimmed(&source[range]).to_owned(),
            }),
            checked,
            target: Target::Top,
        }
    }

    pub fn set_target(&mut self, target: Target) {
        self.target = target;
    }

    pub fn is_saved(&self) -> bool {
        self.saved.is_some()
    }

    /// The saved item's title, as its first Markdown line shows it.
    pub fn title(&self) -> Option<&str> {
        let line = self.saved.as_ref()?.raw.lines().next()?;
        let title = ["- [x] ", "- [X] ", "- [ ] ", "- "]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
            .unwrap_or(line)
            .trim();
        (!title.is_empty()).then_some(title)
    }

    pub(crate) fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// None when the source already holds this item as written.
    pub fn save(&self, source: &str, item: &WorkItem) -> Result<Option<Change>> {
        let Some(start) = self.locate(source)? else {
            return self.append(source, item).map(Some);
        };
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let text = item.markdown(self.checked, newline);
        let end = start + self.saved.as_ref().map_or(0, |saved| saved.raw.len());
        if source[start..end] == text {
            return Ok(None);
        }
        Ok(Some(Change {
            range: start..end,
            text,
            item_start: Some(start),
        }))
    }

    /// Removes the item and, when it was last, the blank lines before it.
    pub fn remove(&self, source: &str) -> Result<Option<Change>> {
        let Some(start) = self.locate(source)? else {
            return Ok(None);
        };
        let range = entries(source)
            .into_iter()
            .find(|entry| entry.range.start == start)
            .context("Worklist item is gone")?
            .range;
        let head = trimmed(&source[..start]);
        let range = if source[range.end..].trim().is_empty() && !head.is_empty() {
            let newline = if source.contains("\r\n") { 2 } else { 1 };
            head.len() + newline..source.len()
        } else {
            range
        };
        Ok(Some(Change {
            range,
            text: String::new(),
            item_start: None,
        }))
    }

    /// Track the item after `change` landed; returns its row index.
    pub fn commit(&mut self, source: &str, change: &Change) -> Option<usize> {
        let (index, range) = change.item_start.and_then(|start| {
            entries(source)
                .into_iter()
                .enumerate()
                .find(|(_, entry)| entry.range.start == start)
                .map(|(index, entry)| (index, entry.range))
        })?;
        self.saved = Some(Saved {
            start: range.start,
            raw: trimmed(&source[range]).to_owned(),
        });
        Some(index)
    }

    fn append(&self, source: &str, item: &WorkItem) -> Result<Change> {
        let insertion = append_item(source, &self.target, item, self.checked)?;
        Ok(Change {
            range: insertion.range,
            text: insertion.text,
            item_start: Some(insertion.block_start),
        })
    }

    /// Where the item starts now, following it if other edits moved it.
    fn locate(&self, source: &str) -> Result<Option<usize>> {
        let Some(saved) = &self.saved else {
            return Ok(None);
        };
        if source.get(saved.start..saved.start + saved.raw.len()) == Some(saved.raw.as_str()) {
            return Ok(Some(saved.start));
        }
        entries(source)
            .into_iter()
            .find(|entry| trimmed(&source[entry.range.clone()]) == saved.raw)
            .map(|entry| Some(entry.range.start))
            .context("This item changed elsewhere; reopen it to keep editing")
    }
}

fn trimmed(text: &str) -> &str {
    text.trim_end_matches(['\r', '\n'])
}

#[cfg(test)]
mod tests;
