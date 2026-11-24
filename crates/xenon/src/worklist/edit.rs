use std::ops::Range;

use anyhow::{Context, Result, anyhow, ensure};

use super::doc::{Doc, Section, Target, parse_parts, safe_append_boundary};
use super::item::WorkItem;

pub(super) struct Insertion {
    pub(super) range: Range<usize>,
    pub(super) text: String,
    pub(super) block_start: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SourceEdit {
    pub(super) range: Range<usize>,
    pub(super) text: String,
}

impl SourceEdit {
    pub(super) fn apply(&self, source: &str) -> String {
        let mut out = source.to_owned();
        out.replace_range(self.range.clone(), &self.text);
        out
    }
}

pub(super) fn newline_of(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

pub(super) fn trimmed(text: &str) -> &str {
    text.trim_end_matches(['\r', '\n'])
}

fn insert_block(source: &str, region_end: usize, block: &str) -> Insertion {
    let nl = newline_of(source);
    let head = trimmed(&source[..region_end]);
    if region_end == source.len() {
        let title = if head.is_empty() { "# Worklist" } else { "" };
        return Insertion {
            range: head.len()..source.len(),
            text: format!("{title}{nl}{nl}{block}{nl}"),
            block_start: head.len() + title.len() + 2 * nl.len(),
        };
    }
    if head.is_empty() {
        return Insertion {
            range: 0..0,
            text: format!("{block}{nl}{nl}"),
            block_start: 0,
        };
    }
    Insertion {
        range: head.len()..head.len(),
        text: format!("{nl}{nl}{block}"),
        block_start: head.len() + 2 * nl.len(),
    }
}

fn region_end(source: &str, sections: &[Section], section: Option<usize>) -> usize {
    match section {
        Some(index) => sections[index].end,
        None => sections.first().map_or(source.len(), |first| first.start),
    }
}

pub(super) fn append_block(source: &str, target: &Target, block: &str) -> Result<Insertion> {
    ensure!(
        safe_append_boundary(source),
        "Worklist ends inside an unfinished Markdown block; edit Markdown before capturing"
    );
    let (sections, _) = parse_parts(source);
    let section = target.resolve(&sections)?;
    Ok(insert_block(
        source,
        region_end(source, &sections, section),
        block,
    ))
}

pub(super) fn append_item(source: &str, target: &Target, item: &WorkItem) -> Result<Insertion> {
    let block = item.markdown(newline_of(source));
    append_block(source, target, &block)
}

pub(super) fn removal_range(source: &str, range: Range<usize>) -> Range<usize> {
    let head = trimmed(&source[..range.start]);
    if source[range.end..].trim().is_empty() && !head.is_empty() {
        head.len() + newline_of(source).len()..source.len()
    } else {
        range
    }
}

fn splice(source: &str, range: &Range<usize>, text: &str) -> String {
    let mut out = source.to_owned();
    out.replace_range(range.clone(), text);
    out
}

fn single_line(title: &str) -> Result<String> {
    let title = title.trim();
    ensure!(!title.is_empty(), "Enter a name");
    ensure!(!title.contains(['\r', '\n']), "Name must be one line");
    Ok(title.to_owned())
}

impl Doc {
    pub(super) fn toggle_item(&self, index: usize) -> Option<String> {
        let entry = self.entries.get(index)?;
        let checked = entry.checked?;
        if !entry.editable {
            return None;
        }
        let raw = &self.source[entry.range.clone()];
        let mark = raw.find(if checked { "[x]" } else { "[ ]" })?;
        let at = entry.range.start + mark + 1;
        let replacement = if checked { " " } else { "x" };
        Some(splice(&self.source, &(at..at + 1), replacement))
    }

    pub(super) fn add_item(&self, target: &Target, title: &str) -> Result<String> {
        let item = WorkItem::new(title, "", Some(false))?;
        ensure!(
            safe_append_boundary(&self.source),
            "Worklist ends inside an unfinished Markdown block; edit Markdown before capturing"
        );
        let section = target.resolve(&self.sections)?;
        let block = item.markdown(newline_of(&self.source));
        let insertion = insert_block(
            &self.source,
            region_end(&self.source, &self.sections, section),
            &block,
        );
        Ok(splice(&self.source, &insertion.range, &insertion.text))
    }

    pub(super) fn remove_item(&self, index: usize) -> Result<String> {
        let entry = self.entries.get(index).context("Worklist item is gone")?;
        let range = removal_range(&self.source, entry.range.clone());
        Ok(splice(&self.source, &range, ""))
    }

    pub(super) fn move_item(&self, index: usize, target_index: usize) -> Result<String> {
        let entry = self.entries.get(index).context("Worklist item is gone")?;
        let targets = self.targets();
        let target = targets
            .get(target_index)
            .ok_or_else(|| anyhow!("Section is gone"))?;
        let raw = trimmed(&self.source[entry.range.clone()]).to_owned();
        let without = splice(
            &self.source,
            &removal_range(&self.source, entry.range.clone()),
            "",
        );
        let insertion = append_block(&without, target, &raw)?;
        Ok(splice(&without, &insertion.range, &insertion.text))
    }

    pub(super) fn add_section(&self, title: &str) -> Result<String> {
        let title = single_line(title)?;
        ensure!(
            safe_append_boundary(&self.source),
            "Worklist ends inside an unfinished Markdown block; edit Markdown before adding a section"
        );
        let insertion = insert_block(&self.source, self.source.len(), &format!("## {title}"));
        Ok(splice(&self.source, &insertion.range, &insertion.text))
    }

    pub(super) fn rename_section(&self, target: &Target, title: &str) -> Result<String> {
        let title = single_line(title)?;
        let index = target
            .resolve(&self.sections)?
            .ok_or_else(|| anyhow!("Pick a section"))?;
        Ok(splice(
            &self.source,
            &self.sections[index].title_range,
            &title,
        ))
    }

    pub(super) fn delete_section(&self, target: &Target) -> Result<String> {
        let index = target
            .resolve(&self.sections)?
            .ok_or_else(|| anyhow!("Pick a section"))?;
        let section = &self.sections[index];
        if section.end < self.source.len() {
            return Ok(splice(&self.source, &(section.start..section.end), ""));
        }
        let head = trimmed(&self.source[..section.start]);
        let keep = if head.is_empty() {
            0
        } else {
            head.len() + newline_of(&self.source).len()
        };
        Ok(splice(&self.source, &(keep..self.source.len()), ""))
    }
}
