//! One directory level for the Files tree: directories first, then names
//! case-insensitively. Every entry is listed (no ignore rules), like the file
//! tree has always done.

use std::path::{Path, PathBuf};

/// An entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// True when the path itself is a symlink (not merely a path under one).
    pub is_symlink: bool,
}

/// The entries of `dir`, sorted; empty when it cannot be read.
pub fn list_dir(dir: &Path) -> Vec<TreeEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<TreeEntry> = read
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let ft = entry.file_type().ok();
            let is_symlink = ft.as_ref().is_some_and(|t| t.is_symlink());
            let is_dir = if is_symlink {
                path.is_dir()
            } else {
                ft.is_some_and(|t| t.is_dir())
            };
            TreeEntry {
                path,
                name,
                is_dir,
                is_symlink,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}
