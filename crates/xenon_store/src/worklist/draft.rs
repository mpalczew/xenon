use super::entries::entries;
use super::sections::{newline_of, trimmed};
use super::{SourceEdit, Target, WorkItem, append_item};
use anyhow::{Context, Result};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Change {
    pub edit: SourceEdit,
    /// Where the item starts after the edit; None once it is removed.
    item_start: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct ItemDraft {
    saved: Option<Saved>,
    checked: bool,
    target: Target,
}

#[derive(Clone, Debug)]
struct Saved {
    start: usize,
    raw: String,
}

impl ItemDraft {
    pub fn existing(source: &str, range: Range<usize>, checked: bool) -> Self {
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

    pub fn title(&self) -> Option<&str> {
        let line = self.saved.as_ref()?.raw.lines().next()?;
        let title = ["- [x] ", "- [X] ", "- [ ] ", "- "]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
            .unwrap_or(line)
            .trim();
        (!title.is_empty()).then_some(title)
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// None when the source already holds this item as written.
    pub fn save(&self, source: &str, item: &WorkItem) -> Result<Option<Change>> {
        let Some(start) = self.locate(source)? else {
            return self.append(source, item).map(Some);
        };
        let text = item.markdown(self.checked, newline_of(source));
        let end = start + self.saved.as_ref().map_or(0, |saved| saved.raw.len());
        if source[start..end] == text {
            return Ok(None);
        }
        Ok(Some(Change {
            edit: SourceEdit {
                range: start..end,
                text,
            },
            item_start: Some(start),
        }))
    }

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
            head.len() + newline_of(source).len()..source.len()
        } else {
            range
        };
        Ok(Some(Change {
            edit: SourceEdit {
                range,
                text: String::new(),
            },
            item_start: None,
        }))
    }

    pub(crate) fn commit(&mut self, source: &str, change: &Change) -> Option<usize> {
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
            edit: SourceEdit {
                range: insertion.range,
                text: insertion.text,
            },
            item_start: Some(insertion.block_start),
        })
    }

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
