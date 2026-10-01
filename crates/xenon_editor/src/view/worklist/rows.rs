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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklist_file::entries::parse;

    const SOURCE: &str =
        "# W\n\n- top\n\n## Now\n\n- [ ] A\n- [ ] B\n\n## Empty\n\n## Next\n\n- [ ] C\n";

    fn rows(folded: &[&str]) -> Vec<Cursor> {
        let parsed = parse(SOURCE);
        let folded = folded.iter().map(|s| s.to_string()).collect();
        visible(&parsed.sections, &parsed.entries, &folded)
    }

    #[test]
    fn rows_interleave_headers_and_items() {
        use Cursor::*;
        assert_eq!(
            rows(&[]),
            [
                Item(0),
                Header(0),
                Item(1),
                Item(2),
                Header(1),
                Header(2),
                Item(3)
            ]
        );
    }

    #[test]
    fn folded_sections_hide_their_items_only() {
        use Cursor::*;
        assert_eq!(
            rows(&["Now"]),
            [Item(0), Header(0), Header(1), Header(2), Item(3)]
        );
    }

    #[test]
    fn a_hidden_selection_settles_on_its_header() {
        let parsed = parse(SOURCE);
        let folded = rows(&["Now"]);
        assert_eq!(
            settle(&folded, Cursor::Item(2), &parsed.entries),
            Cursor::Header(0)
        );
        assert_eq!(
            settle(&folded, Cursor::Item(9), &parsed.entries),
            Cursor::Item(3)
        );
    }

    #[test]
    fn stepping_stops_at_both_ends() {
        let all = rows(&[]);
        assert_eq!(step(&all, Cursor::Item(0), -1), Cursor::Item(0));
        assert_eq!(step(&all, Cursor::Item(0), 1), Cursor::Header(0));
        assert_eq!(step(&all, Cursor::Item(3), 1), Cursor::Item(3));
    }
}
