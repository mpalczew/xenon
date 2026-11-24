use std::ops::Range;

use anyhow::{Context, Result, ensure};

use super::doc::{Doc, Section, parse_parts, safe_append_boundary};
use super::item::ItemText;

pub(super) struct Insertion {
    pub(super) range: Range<usize>,
    pub(super) text: String,
    pub(super) block_start: usize,
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
        Some(index) => sections.get(index).map_or(source.len(), |s| s.end),
        None => sections.first().map_or(source.len(), |first| first.start),
    }
}

fn append_block(source: &str, section: Option<usize>, block: &str) -> Result<Insertion> {
    ensure!(
        safe_append_boundary(source),
        "Worklist ends inside an unfinished Markdown block; edit Markdown before capturing"
    );
    let (sections, _) = parse_parts(source);
    Ok(insert_block(
        source,
        region_end(source, &sections, section),
        block,
    ))
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

    /// The updated source and where the new block starts in it.
    pub(super) fn add_item(
        &self,
        section: Option<usize>,
        item: &ItemText,
    ) -> Result<(String, usize)> {
        let block = item.markdown(newline_of(&self.source));
        let insertion = append_block(&self.source, section, &block)?;
        let after = splice(&self.source, &insertion.range, &insertion.text);
        Ok((after, insertion.block_start))
    }

    pub(super) fn remove_item(&self, index: usize) -> Result<String> {
        let entry = self.entries.get(index).context("Worklist item is gone")?;
        let range = removal_range(&self.source, entry.range.clone());
        Ok(splice(&self.source, &range, ""))
    }

    pub(super) fn move_item(
        &self,
        index: usize,
        section: Option<usize>,
    ) -> Result<(String, usize)> {
        let entry = self.entries.get(index).context("Worklist item is gone")?;
        let raw = trimmed(&self.source[entry.range.clone()]).to_owned();
        let without = splice(
            &self.source,
            &removal_range(&self.source, entry.range.clone()),
            "",
        );
        let insertion = append_block(&without, section, &raw)?;
        Ok((
            splice(&without, &insertion.range, &insertion.text),
            insertion.block_start,
        ))
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

    pub(super) fn rename_section(&self, index: usize, title: &str) -> Result<String> {
        let title = single_line(title)?;
        Ok(splice(
            &self.source,
            &self.sections[index].title_range,
            &title,
        ))
    }

    pub(super) fn delete_section(&self, index: usize) -> String {
        let section = &self.sections[index];
        if section.end < self.source.len() {
            return splice(&self.source, &(section.start..section.end), "");
        }
        let head = trimmed(&self.source[..section.start]);
        let keep = if head.is_empty() {
            0
        } else {
            head.len() + newline_of(&self.source).len()
        };
        splice(&self.source, &(keep..self.source.len()), "")
    }
}
