//! Which filesystem events matter to dirtiness, and which workspace owns a
//! path. Pure path rules; each frontend runs its own watcher.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::gitignore::Gitignore;

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

/// Most directories one workspace may have watched. Past this the frontend
/// falls back to polling instead of growing without bound.
pub const MAX_DIRS: usize = 4096;

/// The directories of one workspace worth a (non-recursive) watch: every
/// directory not excluded by `.gitignore`, the root's `.git` and `.git/refs/**`.
/// Never `target/`, `node_modules`, nested repositories or worktrees.
pub struct DirSet {
    root: PathBuf,
    dirs: HashSet<PathBuf>,
    /// True once the cap was hit or a watch failed: watch nothing more.
    pub full: bool,
}

impl DirSet {
    /// Walk the workspace once. Bounded by `MAX_DIRS`.
    pub fn scan(root: &Path) -> Self {
        let mut set = Self {
            root: root.to_path_buf(),
            dirs: HashSet::new(),
            full: false,
        };
        set.add_tree(root);
        let git = root.join(".git");
        if git.is_dir() {
            set.dirs.insert(git.clone());
            set.add_plain_tree(&git.join("refs"));
        }
        set
    }

    pub fn dirs(&self) -> impl Iterator<Item = &PathBuf> {
        self.dirs.iter()
    }

    /// A path just appeared. If it is a new directory inside a watched one,
    /// return it and its non-ignored subdirectories to watch. Returns nothing
    /// for known paths, so repeated events never walk again.
    pub fn grow(&mut self, path: &Path) -> Vec<PathBuf> {
        if self.full || self.dirs.contains(path) {
            return Vec::new();
        }
        let Some(parent) = path.parent() else {
            return Vec::new();
        };
        if !self.dirs.contains(parent) {
            return Vec::new();
        }
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return Vec::new();
        };
        if !std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
            return Vec::new();
        }
        let mut added = Vec::new();
        if rel.starts_with(".git") {
            if rel.starts_with(".git/refs") {
                added = self.add_plain_tree(path);
            }
        } else if !skipped_name(path) && !is_nested_repo(path) && !ignored(&self.root, path) {
            added = self.add_tree(path);
        }
        added
    }

    /// A watched directory is gone: forget it and everything below it.
    pub fn shrink(&mut self, path: &Path) {
        if self.dirs.contains(path) && !path.exists() {
            self.dirs.retain(|d| !d.starts_with(path));
        }
    }

    /// Insert `dir`, honoring the cap. True if it is newly added.
    fn insert(&mut self, dir: PathBuf) -> bool {
        if self.dirs.len() >= MAX_DIRS {
            self.full = true;
            return false;
        }
        self.dirs.insert(dir)
    }

    /// Add `start` and the non-ignored directories below it.
    fn add_tree(&mut self, start: &Path) -> Vec<PathBuf> {
        let walk = ignore::WalkBuilder::new(start)
            .hidden(false)
            .follow_links(false)
            .git_global(false)
            .require_git(false)
            .filter_entry(|e| {
                e.depth() == 0 || !(skipped_name(e.path()) || is_nested_repo(e.path()))
            })
            .build();
        let mut added = Vec::new();
        for entry in walk.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_dir()) {
                continue;
            }
            let dir = entry.into_path();
            if self.dirs.contains(&dir) {
                continue;
            }
            if self.insert(dir.clone()) {
                added.push(dir);
            }
            if self.full {
                break;
            }
        }
        added
    }

    /// Add `start` and every directory below it, without ignore rules (refs).
    fn add_plain_tree(&mut self, start: &Path) -> Vec<PathBuf> {
        let mut added = Vec::new();
        let mut todo = vec![start.to_path_buf()];
        while let Some(dir) = todo.pop() {
            if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
                continue;
            }
            if !self.dirs.contains(&dir) {
                if !self.insert(dir.clone()) {
                    break;
                }
                added.push(dir.clone());
            }
            if let Ok(read) = std::fs::read_dir(&dir) {
                todo.extend(read.flatten().map(|e| e.path()));
            }
        }
        added
    }
}

/// Directories that are never watched by name (`.git` is added separately).
fn skipped_name(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(".git" | ".worktrees")
    )
}

/// A directory holding its own `.git` is another repository or a worktree.
fn is_nested_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Is `dir` ignored by a `.gitignore` on its way down from the root (or by
/// `.git/info/exclude`)? Judged by path from the root, never by where the
/// event came from. The deepest rule that decides wins.
fn ignored(root: &Path, dir: &Path) -> bool {
    let Ok(rel) = dir.strip_prefix(root) else {
        return false;
    };
    let mut verdict = false;
    let mut decide = |ig: Gitignore| {
        let m = ig.matched(dir, true);
        if m.is_ignore() {
            verdict = true;
        } else if m.is_whitelist() {
            verdict = false;
        }
    };
    decide(Gitignore::new(root.join(".git/info/exclude")).0);
    let mut base = root.to_path_buf();
    decide(Gitignore::new(base.join(".gitignore")).0);
    let parts: Vec<_> = rel.components().collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        base.push(part);
        decide(Gitignore::new(base.join(".gitignore")).0);
    }
    verdict
}
