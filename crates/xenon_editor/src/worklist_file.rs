//! Workspace-local worklist storage shared by capture and the editor.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};

/// The source bytes are the revision. Worklists are intentionally small text files.
pub struct WorklistFile {
    root: PathBuf,
    path: PathBuf,
}

#[derive(Clone)]
pub struct WorklistUndo {
    root: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}

impl WorklistUndo {
    pub fn undo(&self) -> Result<()> {
        let file = WorklistFile::new(&self.root)?;
        ensure!(
            file.read()?.as_deref() == Some(self.after.as_slice()),
            "worklist changed since capture; undo was not applied"
        );
        if let Some(before) = &self.before {
            file.write(Some(&self.after), before)
        } else {
            fs::remove_file(file.path())?;
            Ok(())
        }
    }
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

    pub fn append(&self, text: &str, task: bool) -> Result<PathBuf> {
        self.append_with_undo(text, task).map(|(path, _)| path)
    }

    pub fn append_with_undo(&self, text: &str, task: bool) -> Result<(PathBuf, WorklistUndo)> {
        let old = self.read()?;
        let source = match old.as_ref() {
            Some(bytes) => std::str::from_utf8(bytes).context("worklist is not UTF-8")?,
            None => "",
        };
        let updated = prepare_append(source, text, task)?;
        self.write(old.as_deref(), updated.as_bytes())?;
        Ok((
            self.path.clone(),
            WorklistUndo {
                root: self.root.clone(),
                before: old,
                after: updated.into_bytes(),
            },
        ))
    }

    pub fn write(&self, expected: Option<&[u8]>, updated: &[u8]) -> Result<()> {
        // Re-resolve before mutation; a replaced symlink cannot redirect the write.
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

pub(crate) fn prepare_append(source: &str, text: &str, task: bool) -> Result<String> {
    ensure!(
        safe_append_boundary(source),
        "Worklist ends inside an unfinished Markdown block; edit Markdown before capturing"
    );
    Ok(append_entry(source, text, task))
}

fn append_entry(source: &str, text: &str, task: bool) -> String {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = source.trim_end_matches(['\r', '\n']).to_string();
    if out.is_empty() {
        out.push_str("# Worklist");
    }
    out.push_str(newline);
    out.push_str(newline);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for (index, line) in normalized.lines().enumerate() {
        if index > 0 {
            out.push_str(newline);
            if task {
                out.push_str("  ");
            }
        } else if task {
            out.push_str("- [ ] ");
        }
        out.push_str(line);
    }
    out.push_str(newline);
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_append_rejects_unclosed_fence_without_changing_source() {
        let source = "# Worklist\n\n```md\n";
        assert!(prepare_append(source, "Next", true).is_err());
        assert_eq!(source, "# Worklist\n\n```md\n");
    }

    #[test]
    fn append_keeps_multiline_and_crlf() {
        assert_eq!(
            append_entry("# Worklist\r\n", "Next\nDetail", true),
            "# Worklist\r\n\r\n- [ ] Next\r\n  Detail\r\n"
        );
    }

    #[test]
    fn invalid_fence_closer_does_not_swallow_capture() {
        assert!(!safe_append_boundary("```md\nbody\n```still-code\n"));
        assert!(safe_append_boundary("```md\nbody\n```\n"));
    }

    #[test]
    fn stale_write_is_rejected_without_losing_agent_edit() {
        let root = tempfile::tempdir().unwrap();
        let file = WorklistFile::new(root.path()).unwrap();
        file.write(None, b"original").unwrap();
        let old = file.read().unwrap().unwrap();
        fs::write(file.path(), "agent edit").unwrap();
        assert!(file.write(Some(&old), b"my edit").is_err());
        assert_eq!(fs::read_to_string(file.path()).unwrap(), "agent edit");
    }

    #[test]
    fn capture_undo_restores_only_the_revision_it_created() {
        let root = tempfile::tempdir().unwrap();
        let file = WorklistFile::new(root.path()).unwrap();
        let (_, undo) = file.append_with_undo("First", true).unwrap();
        undo.undo().unwrap();
        assert!(!file.path().exists());
        let (_, undo) = file.append_with_undo("Second", false).unwrap();
        fs::write(file.path(), "agent wrote this").unwrap();
        assert!(undo.undo().is_err());
        assert_eq!(fs::read_to_string(file.path()).unwrap(), "agent wrote this");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_outside_symlinks_for_both_write_paths() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join(".xenon")).unwrap();
        assert!(WorklistFile::new(root.path()).is_err());
    }
}
