//! Worklist sections: `#`/`##` headings after the document title, and the source
//! edits that add to, rename, and remove them. Edits touch only the lines they name.

use super::entries::parse;
use anyhow::{Result, bail, ensure};
use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct Section {
    pub(crate) title: String,
    /// Start of the heading line.
    pub(crate) start: usize,
    /// The heading's text, without its `#` marks.
    pub(crate) title_range: Range<usize>,
    /// First byte after the heading.
    pub(crate) body_start: usize,
    /// Start of the next section's heading, or the end of the source.
    pub(crate) end: usize,
}

impl Section {
    pub(super) fn at(source: &str, heading: Range<usize>) -> Self {
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
        let end = heading.end.max(first_end);
        let body_start = if source[..end].ends_with('\n') {
            end
        } else {
            source[end..]
                .find('\n')
                .map_or(source.len(), |i| end + i + 1)
        };
        Self {
            title: source[text_start..text_end].to_owned(),
            start,
            title_range: text_start..text_end,
            body_start,
            end: source.len(),
        }
    }

    pub(crate) fn is_empty(&self, source: &str) -> bool {
        source[self.body_start.min(self.end)..self.end]
            .trim()
            .is_empty()
    }
}

/// Where a new item goes: above the first section, or at the end of one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Top,
    Section {
        index: usize,
        title: String,
    },
}

impl Target {
    pub fn label(&self) -> &str {
        match self {
            Self::Top => "Top",
            Self::Section { title, .. } if title.is_empty() => "Untitled",
            Self::Section { title, .. } => title,
        }
    }

    pub fn is_section(&self) -> bool {
        matches!(self, Self::Section { .. })
    }

    pub(crate) fn of(sections: &[Section], section: Option<usize>) -> Self {
        match section.and_then(|index| Some((index, sections.get(index)?))) {
            Some((index, found)) => Self::Section {
                index,
                title: found.title.clone(),
            },
            None => Self::Top,
        }
    }

    /// The section index this target names now, following a renumbering by title.
    pub(crate) fn resolve(&self, sections: &[Section]) -> Result<Option<usize>> {
        let Self::Section { index, title } = self else {
            return Ok(None);
        };
        if sections
            .get(*index)
            .is_some_and(|found| found.title == *title)
        {
            return Ok(Some(*index));
        }
        match sections.iter().position(|found| found.title == *title) {
            Some(index) => Ok(Some(index)),
            None => bail!("Section “{title}” is gone; pick another"),
        }
    }
}

/// Where capture can file an item: sections in file order, preceded by the
/// top when something sits above the first section. One entry means no choice.
pub fn capture_targets(source: &str) -> Vec<Target> {
    let parsed = parse(source);
    let mut targets = Vec::new();
    if parsed.sections.is_empty() || parsed.entries.iter().any(|entry| entry.section.is_none()) {
        targets.push(Target::Top);
    }
    targets.extend((0..parsed.sections.len()).map(|i| Target::of(&parsed.sections, Some(i))));
    targets
}

/// `range` becomes `text`; `block_start` is where the inserted block begins.
pub(crate) struct Insertion {
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
    pub(crate) block_start: usize,
}

/// A plain replacement in the source.
pub struct SourceEdit {
    pub range: Range<usize>,
    pub text: String,
}

impl SourceEdit {
    pub fn apply(&self, source: &str) -> String {
        let mut out = source.to_owned();
        out.replace_range(self.range.clone(), &self.text);
        out
    }
}

pub(crate) fn newline_of(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn trimmed(text: &str) -> &str {
    text.trim_end_matches(['\r', '\n'])
}

/// Inserts `block` as the last thing before `region_end`. At the end of the file
/// the tail is normalized to one newline; elsewhere nothing around it changes.
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

pub(crate) fn append_block(source: &str, target: &Target, block: &str) -> Result<Insertion> {
    ensure!(
        super::safe_append_boundary(source),
        "Worklist ends inside an unfinished Markdown block; edit Markdown before capturing"
    );
    let sections = parse(source).sections;
    let section = target.resolve(&sections)?;
    Ok(insert_block(
        source,
        region_end(source, &sections, section),
        block,
    ))
}

/// A new, empty section after `after`'s items; at the end of the file when None.
pub fn insert_section(source: &str, after: Option<&Target>, title: &str) -> Result<SourceEdit> {
    let title = single_line(title)?;
    ensure!(
        super::safe_append_boundary(source),
        "Worklist ends inside an unfinished Markdown block; edit Markdown before adding a section"
    );
    let sections = parse(source).sections;
    let end = match after {
        Some(target) => region_end(source, &sections, target.resolve(&sections)?),
        None => source.len(),
    };
    let insertion = insert_block(source, end, &format!("## {title}"));
    Ok(SourceEdit {
        range: insertion.range,
        text: insertion.text,
    })
}

pub fn rename_section(source: &str, target: &Target, title: &str) -> Result<SourceEdit> {
    let title = single_line(title)?;
    let sections = parse(source).sections;
    let index = target
        .resolve(&sections)?
        .ok_or_else(|| anyhow::anyhow!("Pick a section"))?;
    Ok(SourceEdit {
        range: sections[index].title_range.clone(),
        text: title,
    })
}

/// Removes a section that has nothing under its heading.
pub fn delete_empty_section(source: &str, target: &Target) -> Result<SourceEdit> {
    let sections = parse(source).sections;
    let index = target
        .resolve(&sections)?
        .ok_or_else(|| anyhow::anyhow!("Pick a section"))?;
    let section = &sections[index];
    ensure!(
        section.is_empty(source),
        "“{}” has items. Edit Markdown to remove it.",
        section.title
    );
    if section.end < source.len() {
        return Ok(SourceEdit {
            range: section.start..section.end,
            text: String::new(),
        });
    }
    let head = trimmed(&source[..section.start]);
    let keep = if head.is_empty() {
        0
    } else {
        head.len() + newline_of(source).len()
    };
    Ok(SourceEdit {
        range: keep..source.len(),
        text: String::new(),
    })
}

fn single_line(title: &str) -> Result<String> {
    let title = title.trim();
    ensure!(!title.is_empty(), "Enter a name");
    ensure!(!title.contains(['\r', '\n']), "Name must be one line");
    Ok(title.to_owned())
}

#[cfg(test)]
mod tests;
