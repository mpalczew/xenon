//! Typed paths: `~` expansion and "is this an existing folder" checks.

use std::path::{Path, PathBuf};

/// Expand `~` / `~/…` and absolute paths.
pub fn expand_user_path(input: &str) -> Option<PathBuf> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s == "~" {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest));
    }
    if s.starts_with('/') {
        return Some(PathBuf::from(s));
    }
    None
}

/// Canonical existing directory, or None.
pub fn resolve_existing_dir(path: &Path) -> Option<PathBuf> {
    let path = if path.starts_with("~") {
        expand_user_path(&path.to_string_lossy())?
    } else {
        path.to_path_buf()
    };
    let canon = path.canonicalize().ok()?;
    canon.is_dir().then_some(canon)
}

/// Fast dir check for UI (no canonicalize). Prefer this on the main thread.
pub fn path_is_dir(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    resolve_existing_dir(path).is_some()
}

pub fn same_root(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => false,
    }
}
