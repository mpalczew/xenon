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
