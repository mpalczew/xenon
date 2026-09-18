//! Parent-folder candidates for New Workspace.
//!
//! Path queries (`~/src`, `/Users/…/src/xe`) list that tree, shallower first.
//! Bare names stay with the home walk + fuzzy filter in the view.

use std::path::PathBuf;

use super::{MAX_VISITS, expand_user_path, resolve_existing_dir, should_skip_dir};

const MAX_RELATIVE_DEPTH: usize = 4;
const MAX_PARENT_RESULTS: usize = 40;

/// `Some` when `query` is `~` / `~/…` / absolute. Empty query and bare names
/// are `None` so the view can keep its home-list fuzzy filter.
pub(crate) fn list_parent_candidates(query: &str) -> Option<Vec<PathBuf>> {
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
    let prefix_l = prefix.as_ref().map(|p| p.to_lowercase());
    let mut out = Vec::new();
    if prefix_l.is_none() {
        out.push(root.clone());
    }
    let mut queue = vec![(root, 0usize)];
    let mut qi = 0;
    let mut visits = 0usize;
    while qi < queue.len() && out.len() < MAX_PARENT_RESULTS {
        if visits >= MAX_VISITS {
            break;
        }
        let (dir, depth) = queue[qi].clone();
        qi += 1;
        if depth >= MAX_RELATIVE_DEPTH {
            continue;
        }
        visits += 1;
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                if !file_type.is_dir() {
                    return None;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if should_skip_dir(&name) {
                    None
                } else {
                    Some(entry.path())
                }
            })
            .collect();
        children.sort();
        for child in children {
            if matches_prefix(&child, prefix_l.as_deref()) {
                if out.len() >= MAX_PARENT_RESULTS {
                    break;
                }
                out.push(child.clone());
            }
            if depth + 1 < MAX_RELATIVE_DEPTH {
                queue.push((child, depth + 1));
            }
        }
    }
    out
}

fn matches_prefix(path: &std::path::Path, prefix_l: Option<&str>) -> bool {
    let Some(prefix) = prefix_l else {
        return true;
    };
    path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase().starts_with(prefix))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn rels(root: &Path, out: &[PathBuf]) -> Vec<String> {
        out.iter()
            .filter_map(|p| {
                p.strip_prefix(root)
                    .ok()
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
            })
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn tree() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        fs::create_dir_all(root.join("a/deep/nested")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::create_dir_all(root.join("xenon")).unwrap();
        fs::create_dir_all(root.join("other/xenon-extra")).unwrap();
        (tmp, root)
    }

    #[test]
    fn bare_name_is_not_path_scoped() {
        assert!(list_parent_candidates("crypto").is_none());
        assert!(list_parent_candidates("").is_none());
        assert!(list_parent_candidates("   ").is_none());
    }

    #[test]
    fn existing_dir_lists_self_then_shallow_first() {
        let (_tmp, root) = tree();
        let out = list_parent_candidates(&root.to_string_lossy()).expect("path query");
        assert_eq!(out[0], root);
        let names = rels(&root, &out);
        let a = names.iter().position(|n| n == "a").expect("a");
        let b = names.iter().position(|n| n == "b").expect("b");
        let deep = names.iter().position(|n| n == "a/deep").expect("a/deep");
        let nested = names
            .iter()
            .position(|n| n == "a/deep/nested")
            .expect("nested");
        assert!(a < deep, "depth 1 before depth 2: {names:?}");
        assert!(b < deep, "sibling before nested: {names:?}");
        assert!(deep < nested, "depth 2 before depth 3: {names:?}");
    }

    #[test]
    fn missing_last_segment_prefix_filters() {
        let (_tmp, root) = tree();
        let query = root.join("xe");
        let out = list_parent_candidates(&query.to_string_lossy()).expect("path query");
        let names = rels(&root, &out);
        assert!(
            names.iter().any(|n| n == "xenon"),
            "expected xenon in {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "other/xenon-extra"),
            "expected nested prefix in {names:?}"
        );
        assert!(
            !names.iter().any(|n| n == "other" || n == "a" || n == "b"),
            "non-matching siblings leaked: {names:?}"
        );
        let xenon = names.iter().position(|n| n == "xenon").unwrap();
        let extra = names.iter().position(|n| n == "other/xenon-extra").unwrap();
        assert!(xenon < extra, "shallower prefix first: {names:?}");
    }
}
