//! One item carried across autosaves: appended on the first save, rewritten in place after.

use super::entries::entries;
use super::{WorkItem, append_item};
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
        }
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

    pub fn forget(&mut self) {
        self.saved = None;
    }

    fn append(&self, source: &str, item: &WorkItem) -> Result<Change> {
        let updated = append_item(source, item, self.checked)?;
        let item_start = entries(&updated).last().map(|entry| entry.range.start);
        let keep = trimmed(source).len();
        let keep = if updated.starts_with(&source[..keep]) {
            keep
        } else {
            0
        };
        Ok(Change {
            range: keep..source.len(),
            text: updated[keep..].to_owned(),
            item_start,
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
mod tests {
    use super::*;

    fn item(title: &str, details: &str) -> WorkItem {
        WorkItem::new(title, details, true).unwrap()
    }

    fn land(draft: &mut ItemDraft, source: &str, item: &WorkItem) -> String {
        let change = draft.save(source, item).unwrap().unwrap();
        let updated = change.apply(source);
        draft.commit(&updated, &change);
        updated
    }

    #[test]
    fn title_reads_the_saved_first_line() {
        let mut draft = ItemDraft::default();
        assert_eq!(draft.title(), None);
        land(&mut draft, "# Worklist\n", &item("Fix focus", "Right pane"));
        assert_eq!(draft.title(), Some("Fix focus"));
    }

    #[test]
    fn first_save_appends_then_later_saves_rewrite_in_place() {
        let mut draft = ItemDraft::default();
        let source = "# Worklist\n\n- [ ] Old\n";
        let source = land(&mut draft, source, &item("F", ""));
        assert_eq!(source, "# Worklist\n\n- [ ] Old\n\n- [ ] F\n");
        let source = land(&mut draft, &source, &item("Fix", "why"));
        assert_eq!(source, "# Worklist\n\n- [ ] Old\n\n- [ ] Fix\n  - why\n");
        assert!(draft.save(&source, &item("Fix", "why")).unwrap().is_none());
    }

    #[test]
    fn rewrite_keeps_checked_state_and_crlf() {
        let source = "- [x] First\r\n  - Detail\r\n\r\nNext note.\r\n";
        let range = entries(source)[0].range.clone();
        let mut draft = ItemDraft::existing(source, range, true);
        let updated = land(&mut draft, source, &item("Revised", "More"));
        assert_eq!(updated, "- [x] Revised\r\n  - More\r\n\r\nNext note.\r\n");
    }

    #[test]
    fn follows_the_item_when_an_agent_edits_above_it() {
        let mut draft = ItemDraft::default();
        let source = "# Worklist\n";
        let source = land(&mut draft, source, &item("Mine", ""));
        let source = source.replace("# Worklist\n", "# Worklist\n\n- [ ] Agent\n");
        let updated = land(&mut draft, &source, &item("Mine!", ""));
        assert_eq!(updated, "# Worklist\n\n- [ ] Agent\n\n- [ ] Mine!\n");
    }

    #[test]
    fn refuses_to_write_over_an_item_changed_elsewhere() {
        let mut draft = ItemDraft::default();
        let source = "# Worklist\n";
        let source = land(&mut draft, source, &item("Mine", ""));
        let source = source.replace("Mine", "Theirs");
        assert!(draft.save(&source, &item("Mine!", "")).is_err());
    }

    #[test]
    fn removing_the_last_item_restores_the_source() {
        let mut draft = ItemDraft::default();
        let source = "# Worklist\n";
        let added = land(&mut draft, source, &item("Gone", ""));
        assert_eq!(draft.remove(&added).unwrap().unwrap().apply(&added), source);
    }
}
