use std::path::PathBuf;

use super::{expand_user_path, resolve_existing_dir};

const MAX_RELATIVE_DEPTH: usize = 4;
const MAX_PARENT_RESULTS: usize = 40;

/// `Some` when `query` is `~` / `~/…` / absolute. Empty query and bare names
/// are `None` so the view can keep its home-list fuzzy filter.
pub fn list_parent_candidates(query: &str) -> Option<Vec<PathBuf>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let expanded = expand_user_path(q)?;
    Some(list_under_path(expanded))
}

fn list_under_path(expanded: PathBuf) -> Vec<PathBuf> {
    if let Some(root) = resolve_existing_dir(&expanded) {
        return bfs_descendants(root, None);
    }
    let Some(name) = expanded.file_name() else {
        return Vec::new();
    };
    let prefix = name.to_string_lossy().into_owned();
    if prefix.is_empty() {
        return Vec::new();
    }
    let Some(parent) = expanded.parent() else {
        return Vec::new();
    };
    let Some(parent) = resolve_existing_dir(parent) else {
        return Vec::new();
    };
    bfs_descendants(parent, Some(prefix))
}

fn bfs_descendants(root: PathBuf, prefix: Option<String>) -> Vec<PathBuf> {
    let prefix_l = prefix.map(|p| p.to_lowercase());
    let found = crate::walk::subdirs(&root, MAX_RELATIVE_DEPTH)
        .into_iter()
        .filter(|path| {
            prefix_l.as_deref().is_none_or(|prefix| {
                path.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with(prefix))
            })
        });
    let own_root = prefix_l.is_none().then(|| root.clone());
    own_root
        .into_iter()
        .chain(found)
        .take(MAX_PARENT_RESULTS)
        .collect()
}
