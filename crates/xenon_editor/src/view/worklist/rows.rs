//! The worklist's selectable rows: section headers and the items under them.

use crate::worklist_file::entries::Entry;
use crate::worklist_file::sections::Section;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::view) enum Cursor {
    Item(usize),
    Header(usize),
}

/// Rows in reading order: items above the first section, then each header
/// followed by its items unless the section is folded.
pub(super) fn visible(
    sections: &[Section],
    entries: &[Entry],
    folded: &BTreeSet<String>,
) -> Vec<Cursor> {
    let items = |section: Option<usize>| {
        entries
            .iter()
            .enumerate()
            .filter(move |(_, entry)| entry.section == section)
            .map(|(index, _)| Cursor::Item(index))
    };
    let mut rows: Vec<Cursor> = items(None).collect();
    for (index, section) in sections.iter().enumerate() {
        rows.push(Cursor::Header(index));
        if !folded.contains(&section.title) {
            rows.extend(items(Some(index)));
        }
    }
    rows
}

/// The nearest valid row when edits, folds, or external changes moved the cursor.
pub(super) fn settle(rows: &[Cursor], cursor: Cursor, entries: &[Entry]) -> Cursor {
    if rows.contains(&cursor) {
        return cursor;
    }
    if let Cursor::Item(index) = cursor
        && let Some(section) = entries.get(index).and_then(|entry| entry.section)
        && rows.contains(&Cursor::Header(section))
    {
        return Cursor::Header(section);
    }
    let same_kind = |row: &&Cursor| std::mem::discriminant(*row) == std::mem::discriminant(&cursor);
    rows.iter()
        .rev()
        .find(same_kind)
        .or(rows.last())
        .copied()
        .unwrap_or(Cursor::Item(0))
}

pub(super) fn step(rows: &[Cursor], cursor: Cursor, delta: isize) -> Cursor {
    let at = rows.iter().position(|row| *row == cursor).unwrap_or(0) as isize;
    let next = (at + delta).clamp(0, rows.len().saturating_sub(1) as isize);
    rows.get(next as usize).copied().unwrap_or(cursor)
}
