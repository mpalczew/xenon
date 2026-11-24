use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail, ensure};

use super::item::{Draft, DraftEdit};

pub(super) struct UndoPair {
    pub(super) before: String,
    pub(super) after: String,
}

pub(super) enum Update {
    Saved(UndoPair),
    Stale(String),
    Noop,
}

struct WorklistFile {
    root: PathBuf,
    path: PathBuf,
}

impl WorklistFile {
    fn new(workspace_root: &Path) -> Result<Self> {
        let root = fs::canonicalize(workspace_root).with_context(|| {
            format!(
                "workspace root {} is not readable",
                workspace_root.display()
            )
        })?;
        let dir = root.join(".xenon");
        if dir.exists() {
            let resolved_dir = fs::canonicalize(&dir)?;
            ensure!(
                resolved_dir.starts_with(&root),
                ".xenon resolves outside this workspace"
            );
        }
        let requested = dir.join("worklist.md");
        let path = match fs::symlink_metadata(&requested) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = fs::canonicalize(&requested)?;
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

    fn read(&self) -> Result<Option<Vec<u8>>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn write(&self, expected: Option<&[u8]>, updated: &[u8]) -> Result<()> {
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

pub(super) fn ensure_exists(root: &Path) -> Result<()> {
    let file = WorklistFile::new(root)?;
    match file.read()? {
        Some(_) => Ok(()),
        None => file.write(None, b"# Worklist\n"),
    }
}

pub(super) fn read_text(root: &Path) -> String {
    WorklistFile::new(root)
        .and_then(|file| file.read())
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

pub(super) fn commit_draft(
    root: &Path,
    draft: &mut Draft,
    edit: DraftEdit<'_>,
) -> Result<Option<UndoPair>> {
    let file = WorklistFile::new(root)?;
    let old = file.read()?;
    let before = match old.as_deref() {
        Some(bytes) => std::str::from_utf8(bytes)?.to_owned(),
        None => String::new(),
    };
    let Some(write) = draft.prepare(&before, edit)? else {
        return Ok(None);
    };
    let after = write.updated.clone();
    file.write(old.as_deref(), after.as_bytes())?;
    draft.commit(write);
    Ok(Some(UndoPair { before, after }))
}

pub(super) fn try_update(
    root: &Path,
    expected: &str,
    change: impl FnOnce() -> Result<Option<String>>,
) -> Result<Update> {
    let file = WorklistFile::new(root)?;
    let old = file.read()?;
    let before = String::from_utf8(old.clone().unwrap_or_default())?;
    if before != expected {
        return Ok(Update::Stale(before));
    }
    let Some(after) = change()? else {
        return Ok(Update::Noop);
    };
    file.write(old.as_deref(), after.as_bytes())
        .map_err(|error| anyhow!("Worklist change was not saved: {error}"))?;
    Ok(Update::Saved(UndoPair { before, after }))
}

pub(super) fn revert(root: &Path, undo: &UndoPair) -> Result<()> {
    let file = WorklistFile::new(root)?;
    let old = file.read()?;
    ensure!(
        old.as_deref() == Some(undo.after.as_bytes()),
        "the worklist changed since"
    );
    file.write(old.as_deref(), undo.before.as_bytes())
}
