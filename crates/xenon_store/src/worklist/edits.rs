use std::ops::Range;

use anyhow::{Context, Result};

use super::{ItemDraft, Target, WorkItem, append_item, sections};

pub fn add_item(source: &str, target: &Target, title: &str) -> Result<String> {
    let item = WorkItem::new(title, "", true)?;
    let insertion = append_item(source, target, &item, false)?;
    Ok(splice(source, &insertion.range, &insertion.text))
}

pub fn remove_item(source: &str, range: Range<usize>) -> Result<String> {
    let change = ItemDraft::existing(source, range, false)
        .remove(source)?
        .context("Worklist item is gone")?;
    Ok(change.apply(source))
}

pub fn move_item(source: &str, range: Range<usize>, target: &Target) -> Result<String> {
    let raw = source[range.clone()]
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    let without = remove_item(source, range)?;
    let insertion = sections::append_block(&without, target, &raw)?;
    Ok(splice(&without, &insertion.range, &insertion.text))
}

fn splice(source: &str, range: &Range<usize>, text: &str) -> String {
    let mut out = source.to_owned();
    out.replace_range(range.clone(), text);
    out
}
