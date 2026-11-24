use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};

pub(super) struct WorklistFile {
    root: PathBuf,
    path: PathBuf,
}

impl WorklistFile {
    pub(super) fn new(workspace_root: &Path) -> Result<Self> {
        let root = fs::canonicalize(workspace_root).with_context(|| {
            format!(
                "workspace root {} is not readable",
                workspace_root.display()
            )
        })?;
        let requested = root.join(super::PATH);
        let dir = requested.parent().context("worklist has no parent")?;
        if dir.exists() {
            let resolved_dir = fs::canonicalize(dir)?;
            ensure!(
                resolved_dir.starts_with(&root),
                ".xenon resolves outside this workspace"
            );
        }
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

    pub(super) fn read(&self) -> Result<Option<Vec<u8>>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes if the file still holds `expected`; `false` means someone else wrote first.
    pub(super) fn write(&self, expected: Option<&[u8]>, updated: &[u8]) -> Result<bool> {
        let current = Self::new(&self.root)?;
        ensure!(current.path == self.path, "worklist path changed; retry");
        if current.read()?.as_deref() != expected {
            return Ok(false);
        }
        let directory = self.path.parent().context("worklist has no parent")?;
        fs::create_dir_all(directory)?;
        let permissions = fs::metadata(&self.path).ok().map(|m| m.permissions());
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp = directory.join(format!(".worklist-{}-{nonce}.tmp", std::process::id()));
        let result = (|| -> Result<bool> {
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
            if self.read()?.as_deref() != expected {
                return Ok(false);
            }
            fs::rename(&temp, &self.path)?;
            Ok(true)
        })();
        if !matches!(result, Ok(true)) {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
