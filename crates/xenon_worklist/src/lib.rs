//! The worklist file (`<root>/.xenon/worklist.md`): parse, change, undo. One owner for
//! every writer (desktop list, capture dialog, phone). Items are addressed by content
//! keys, so an op made against an old view still means the same thing.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

mod doc;
mod edit;
mod file;
mod item;
mod key;
mod plan;

use doc::Doc;
use file::WorklistFile;
use plan::{Step, plan};

pub use item::{ItemText, TITLE_LIMIT, title_length};
pub use key::{ItemKey, Place, SectionKey};

pub const PATH: &str = ".xenon/worklist.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub place: Place,
    pub label: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub key: ItemKey,
    pub title: String,
    /// Indented point bullets, one per line, two spaces per depth.
    pub details: String,
    /// `None` = a note, not a task.
    pub done: Option<bool>,
    /// False when the item holds Markdown the list can't safely rewrite.
    pub editable: bool,
}

impl Snapshot {
    fn of(doc: &Doc) -> Self {
        let groups = doc
            .regions()
            .into_iter()
            .map(|region| {
                let (place, label) = match region.map(|i| &doc.sections[i]) {
                    None => (Place::Top, "Top"),
                    Some(section) if section.title.is_empty() => {
                        (Place::Section(section.key.clone()), "Untitled")
                    }
                    Some(section) => (Place::Section(section.key.clone()), &*section.title),
                };
                let items = doc
                    .entries
                    .iter()
                    .filter(|entry| entry.section == region)
                    .map(|entry| Item {
                        key: entry.key.clone(),
                        title: entry.title.clone(),
                        details: entry.details.clone(),
                        done: entry.checked,
                        editable: entry.editable,
                    })
                    .collect();
                Group {
                    place,
                    label: label.to_owned(),
                    items,
                }
            })
            .collect();
        Self { groups }
    }
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::of(&Doc::default())
    }
}

/// Every change the worklist supports. Each states the end state it wants, so running
/// it twice is safe.
#[derive(Clone, Debug)]
pub enum Op {
    SetDone { item: ItemKey, done: bool },
    Add { to: Place, text: ItemText },
    Replace { item: ItemKey, text: ItemText },
    Remove { item: ItemKey },
    Move { item: ItemKey, to: Place },
    AddSection { title: String },
    RenameSection { section: SectionKey, title: String },
    DeleteSection { section: SectionKey },
}

/// What `apply` did. `now` is the file as it is after the call: render it.
pub struct Applied {
    pub now: Snapshot,
    pub effect: Effect,
}

pub enum Effect {
    /// Written. `item` is the added, replaced or moved item, for callers that keep
    /// tracking it. `undo` reverts this exact write.
    Saved { item: Option<ItemKey>, undo: Undo },
    /// Already in the requested state.
    Unchanged,
    /// The item or section no longer exists with that content. Nothing written.
    Gone,
}

/// Opaque before/after pair; `revert` succeeds only if the file still equals `after`.
pub struct Undo {
    before: String,
    after: String,
}

/// Reads and parses. A missing or non-UTF-8 file reads as empty. Never writes.
pub fn read(workspace_root: &Path) -> Result<Snapshot> {
    let bytes = WorklistFile::new(workspace_root)?.read()?;
    let source = String::from_utf8(bytes.unwrap_or_default()).unwrap_or_default();
    Ok(Snapshot::of(&Doc::parse(source)))
}

/// The one way to change the worklist. Errors are user-facing refusals or I/O failures;
/// staleness is never an error.
pub fn apply(workspace_root: &Path, op: Op) -> Result<Applied> {
    let file = WorklistFile::new(workspace_root)?;
    for _ in 0..3 {
        let bytes = file.read()?;
        let doc = Doc::parse(String::from_utf8(bytes.clone().unwrap_or_default())?);
        let (now, effect) = match plan(&doc, &op)? {
            Step::Unchanged => (Snapshot::of(&doc), Effect::Unchanged),
            Step::Gone => (Snapshot::of(&doc), Effect::Gone),
            Step::Write { after, item } => {
                if !file.write(bytes.as_deref(), after.as_bytes())? {
                    continue;
                }
                let now = Snapshot::of(&Doc::parse(after.clone()));
                let undo = Undo {
                    before: doc.source,
                    after,
                };
                (now, Effect::Saved { item, undo })
            }
        };
        return Ok(Applied { now, effect });
    }
    bail!("The worklist is busy; try again")
}

pub fn revert(workspace_root: &Path, undo: &Undo) -> Result<()> {
    let file = WorklistFile::new(workspace_root)?;
    let current = file.read()?;
    if current.as_deref() != Some(undo.after.as_bytes())
        || !file.write(current.as_deref(), undo.before.as_bytes())?
    {
        bail!("the worklist changed since");
    }
    Ok(())
}

/// Creates the file with "# Worklist" if missing; returns its path for the editor.
pub fn ensure(workspace_root: &Path) -> Result<PathBuf> {
    let file = WorklistFile::new(workspace_root)?;
    if file.read()?.is_none() {
        file.write(None, b"# Worklist\n")?;
    }
    Ok(workspace_root.join(PATH))
}
