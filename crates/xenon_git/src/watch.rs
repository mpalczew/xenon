//! Which filesystem events matter to dirtiness, and which workspace owns a
//! path. Pure path rules; each frontend runs its own watcher.

use std::path::{Path, PathBuf};

/// Drop high-churn paths that never affect dirt for normal projects.
pub fn is_noise(path: &Path) -> bool {
    if is_git_meta(path) {
        return false;
    }
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("target" | "node_modules" | "dist" | ".next" | "__pycache__" | ".git")
        )
    })
}

/// Index/HEAD/refs under `.git` affect dirtiness; object packs do not.
pub fn is_git_meta(path: &Path) -> bool {
    let mut parts = path.components().filter_map(|c| c.as_os_str().to_str());
    while let Some(part) = parts.next() {
        if part != ".git" {
            continue;
        }
        return matches!(
            parts.next(),
            None | Some("index" | "HEAD" | "FETCH_HEAD" | "MERGE_HEAD" | "refs" | "COMMIT_EDITMSG")
        );
    }
    false
}

/// The deepest root that contains `path` (nested workspaces pick the inner one).
pub fn owner_of<Id: Copy>(roots: &[(Id, PathBuf)], path: &Path) -> Option<Id> {
    roots
        .iter()
        .filter(|(_, root)| path.starts_with(root))
        .max_by_key(|(_, root)| root.as_os_str().len())
        .map(|(id, _)| *id)
}

/// Directories worth a non-recursive watch under `start`: the ones git does
/// not ignore (`.gitignore`, `.git/info/exclude`, global excludes), without
/// nested repositories and worktrees. Recursive watches would add one inotify
/// watch per directory of `target/` and `node_modules/` as well.
/// `start` itself is always included. For a workspace root (`is_root`) this
/// also includes the `.git` bits that matter: the directory itself (HEAD,
/// index) and the `refs` tree, never `objects`.
pub fn watch_dirs(start: &Path, is_root: bool) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let walk = ignore::WalkBuilder::new(start)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            // A nested repo or worktree has its own `.git`; it is not ours.
            !is_dir
                || entry.depth() == 0
                || (entry.file_name() != ".git" && !entry.path().join(".git").exists())
        })
        .build();
    for entry in walk.flatten() {
        let path = entry.path();
        if entry.file_type().is_some_and(|t| t.is_dir())
            && (path != start || is_root || !path.join(".git").exists())
        {
            dirs.push(path.to_path_buf());
        }
    }
    if is_root {
        let git = start.join(".git");
        if git.is_dir() {
            dirs.push(git.clone());
            let refs = ignore::WalkBuilder::new(git.join("refs"))
                .standard_filters(false)
                .build();
            dirs.extend(
                refs.flatten()
                    .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
                    .map(ignore::DirEntry::into_path),
            );
        }
    }
    dirs
}
