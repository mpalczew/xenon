//! Workspace-local worklist storage shared by capture and the editor.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
mod draft;
pub mod entries;
mod item;
pub mod sections;
pub mod view;
pub use draft::{Change, ItemDraft};
pub use item::{TITLE_LIMIT, WorkItem, title_length};
pub use sections::{
    SourceEdit, Target, capture_targets, delete_empty_section, insert_section, rename_section,
};

/// Quick-capture chord, shown in the empty list. The app's command catalog asserts it matches.
pub const CAPTURE_KEYS: &str = "⌘⇧K";

/// The source bytes are the revision. Worklists are intentionally small text files.
pub struct WorklistFile {
    root: PathBuf,
    path: PathBuf,
}

impl WorklistFile {
    pub fn new(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let directory = root.join(".xenon");
        if let Ok(metadata) = fs::symlink_metadata(&directory) {
            ensure!(
                metadata.is_dir() || metadata.file_type().is_symlink(),
                ".xenon is not a directory"
            );
            ensure!(
                directory.canonicalize()?.starts_with(&root),
                ".xenon resolves outside this workspace"
            );
        }
        let requested = directory.join("worklist.md");
        let path = match fs::symlink_metadata(&requested) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = requested.canonicalize()?;
                ensure!(
                    resolved.starts_with(&root),
                    "worklist resolves outside this workspace"
                );
                resolved
            }
            Ok(metadata) if metadata.is_file() => requested,
            Ok(_) => bail!("worklist is not a file"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => requested,
            Err(error) => return Err(error.into()),
        };
        Ok(Self { root, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> Result<Option<Vec<u8>>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn write(&self, expected: Option<&[u8]>, updated: &[u8]) -> Result<()> {
        let current = Self::new(&self.root)?;
        ensure!(current.path == self.path, "worklist path changed; retry");
        ensure!(
            current.read()?.as_deref() == expected,
            "worklist changed on disk; retry after reloading"
        );
        let directory = self.path.parent().context("worklist has no parent")?;
        fs::create_dir_all(directory)?;
        let permissions = fs::metadata(&self.path).ok().map(|m| m.permissions());
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp = directory.join(format!(".worklist-{}-{nonce}.tmp", std::process::id()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(updated)?;
            file.sync_all()?;
            drop(file);
            if let Some(permissions) = permissions {
                fs::set_permissions(&temp, permissions)?;
            }
            ensure!(
                Self::new(&self.root)?.path == self.path,
                "worklist path changed; retry"
            );
            ensure!(
                self.read()?.as_deref() == expected,
                "worklist changed on disk; retry after reloading"
            );
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

pub fn append_item(
    source: &str,
    target: &Target,
    item: &WorkItem,
    checked: bool,
) -> Result<Insertion> {
    let block = item.markdown(checked, sections::newline_of(source));
    sections::append_block(source, target, &block)
}

fn safe_append_boundary(source: &str) -> bool {
    if source.lines().next() == Some("---")
        && !source
            .lines()
            .skip(1)
            .any(|line| line == "---" || line == "...")
    {
        return false;
    }
    let mut fence: Option<(char, usize)> = None;
    for line in source.lines() {
        let line = line.trim_start();
        let Some(marker) = line.chars().next().filter(|c| *c == '`' || *c == '~') else {
            continue;
        };
        let count = line.chars().take_while(|c| *c == marker).count();
        if count < 3 {
            continue;
        }
        match fence {
            None => fence = Some((marker, count)),
            Some((open, size))
                if marker == open && count >= size && line[count..].trim().is_empty() =>
            {
                fence = None
            }
            _ => {}
        }
    }
    fence.is_none()
}

pub enum Edit<'a> {
    Save(&'a WorkItem),
    Remove,
}

/// Writes `edit` for `draft` into the workspace's worklist file.
pub fn write_draft(root: &Path, draft: &mut ItemDraft, edit: &Edit) -> anyhow::Result<()> {
    let file = WorklistFile::new(root)?;
    let old = file.read()?;
    let source = match old.as_deref() {
        Some(bytes) => std::str::from_utf8(bytes)?,
        None => "",
    };
    let change: Option<Change> = match edit {
        Edit::Save(item) => draft.save(source, item)?,
        Edit::Remove => draft.remove(source)?,
    };
    let Some(change) = change else { return Ok(()) };
    let updated = change.apply(source);
    file.write(old.as_deref(), updated.as_bytes())?;
    draft.commit(&updated, &change);
    Ok(())
}

/// The worklist as capture would see it; empty when there is none.
pub fn read_source(root: &Path) -> String {
    WorklistFile::new(root)
        .and_then(|file| file.read())
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

pub(crate) use sections::Insertion;
