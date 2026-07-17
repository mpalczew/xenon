//! Directory walk for the file index: gitignore-aware, includes hidden files,
//! skips heavy/dependency trees (VS Code-style semantic excludes, not all-dot).

use std::path::Path;
use std::time::Instant;

use ignore::WalkBuilder;

use crate::Entry;

/// Emit a partial snapshot at least this often while walking.
const EMIT_EVERY_ENTRIES: usize = 400;
/// Or at least this often in wall time (first paint stays snappy).
const EMIT_EVERY_MS: u128 = 50;

/// Collect files/dirs under `root`. Calls `on_partial` with a growing snapshot;
/// return `false` from the callback to abort early (e.g. reindex cancelled).
pub(crate) fn walk_with_progress(
    root: &Path,
    mut on_partial: impl FnMut(&[Entry]) -> bool,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut since_emit = 0usize;
    let mut last_emit = Instant::now();
    let mut emitted_once = false;

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(keep_entry)
        .build();

    for entry in walker.flatten() {
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() && !file_type.is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        entries.push(Entry {
            path: relative.to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
        });
        since_emit += 1;

        let due = !emitted_once
            || since_emit >= EMIT_EVERY_ENTRIES
            || last_emit.elapsed().as_millis() >= EMIT_EVERY_MS;
        if due {
            if !on_partial(&entries) {
                return entries;
            }
            since_emit = 0;
            last_emit = Instant::now();
            emitted_once = true;
        }
    }

    // Final snapshot (even if identical to last partial).
    let _ = on_partial(&entries);
    entries
}

fn keep_entry(entry: &ignore::DirEntry) -> bool {
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
        return true;
    };
    // Only skip directories by semantic name; files like `.claude.json` stay.
    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
    if is_dir && should_skip_dir(name) {
        return false;
    }
    true
}

/// Heavy / dependency / OS dumpsters — never useful as cmd-p destinations at scale.
/// Deliberately not "all dot dirs": `.github`, `.claude`, `.config` stay walkable.
pub(crate) fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        // VCS / package managers
        ".git"
            | ".svn"
            | ".hg"
            | "node_modules"
            | "bower_components"
            | "vendor"
            | "Pods"
            | ".bundle"
            // Build outputs
            | "target"
            | "dist"
            | "build"
            | "out"
            | "coverage"
            | ".next"
            | ".nuxt"
            | ".turbo"
            | ".parcel-cache"
            | ".sass-cache"
            // Tool caches / SDKs (home-scale killers)
            | ".cache"
            | ".cargo"
            | ".rustup"
            | ".npm"
            | ".nvm"
            | ".yarn"
            | ".pnpm-store"
            | ".gradle"
            | ".android"
            | ".local"
            | ".bun"
            | ".rbenv"
            | ".pub-cache"
            | ".konan"
            | ".dartServer"
            | ".dart-tool"
            | ".cocoapods"
            // macOS system trees under $HOME
            | "Library"
            | "Applications"
            | ".Trash"
            | "DerivedData"
            // Python
            | "__pycache__"
            | ".tox"
            | ".venv"
            | "venv"
            | ".mypy_cache"
            | ".pytest_cache"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_dependency_dirs_not_dotfiles() {
        assert!(should_skip_dir("node_modules"));
        assert!(should_skip_dir("target"));
        assert!(should_skip_dir("Library"));
        assert!(should_skip_dir(".git"));
        assert!(should_skip_dir(".cache"));
        assert!(!should_skip_dir(".claude"));
        assert!(!should_skip_dir(".github"));
        assert!(!should_skip_dir("src"));
    }
}
