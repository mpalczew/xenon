use anyhow::{Result, bail, ensure};

use super::Op;
use super::doc::Doc;
use super::edit::{newline_of, trimmed};
use super::key::ItemKey;

pub(super) enum Step {
    Unchanged,
    Gone,
    Write {
        after: String,
        item: Option<ItemKey>,
    },
}

fn write(after: String) -> Step {
    Step::Write { after, item: None }
}

/// The block that now starts at `start`, as the item the write produced.
fn written((after, start): (String, usize)) -> Step {
    let item = Doc::parse(after.clone())
        .entries
        .into_iter()
        .find(|entry| entry.range.start == start)
        .map(|entry| entry.key);
    Step::Write { after, item }
}

/// Pure: what `op` does to `doc`. Keys resolve against `doc`; `Err` is a refusal.
pub(super) fn plan(doc: &Doc, op: &Op) -> Result<Step> {
    Ok(match op {
        Op::SetDone { item, done } => {
            let Some(index) = doc.find(item) else {
                return Ok(Step::Gone);
            };
            if doc.entries[index].checked == Some(*done) {
                return Ok(Step::Unchanged);
            }
            match doc.toggle_item(index) {
                Some(after) => write(after),
                None => bail!("Edit this one in Markdown"),
            }
        }
        Op::Add { to, text } => {
            let Some(section) = doc.region(to) else {
                return Ok(Step::Gone);
            };
            written(doc.add_item(section, text)?)
        }
        Op::Replace { item, text } => {
            let Some(index) = doc.find(item) else {
                return Ok(Step::Gone);
            };
            let entry = &doc.entries[index];
            ensure!(entry.editable, "Edit this one in Markdown");
            let start = entry.range.start;
            let raw = trimmed(&doc.source[entry.range.clone()]);
            let block = text.markdown(newline_of(&doc.source));
            if raw == block {
                return Ok(Step::Unchanged);
            }
            let mut after = doc.source.clone();
            after.replace_range(start..start + raw.len(), &block);
            written((after, start))
        }
        Op::Remove { item } => match doc.find(item) {
            Some(index) => write(doc.remove_item(index)?),
            None => Step::Unchanged,
        },
        Op::Move { item, to } => {
            let (Some(index), Some(section)) = (doc.find(item), doc.region(to)) else {
                return Ok(Step::Gone);
            };
            if doc.entries[index].section == section {
                return Ok(Step::Unchanged);
            }
            written(doc.move_item(index, section)?)
        }
        Op::AddSection { title } => write(doc.add_section(title)?),
        Op::RenameSection { section, title } => match doc.section_index(section) {
            Some(index) => write(doc.rename_section(index, title)?),
            None => Step::Gone,
        },
        Op::DeleteSection { section } => match doc.section_index(section) {
            Some(index) => write(doc.delete_section(index)),
            None => Step::Gone,
        },
    })
}
