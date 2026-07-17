//! Fuzzy file finder for cmd-p: walk a directory (respecting .gitignore, including
//! hidden/dot files but not heavy trees) into a shareable `FileIndex`, then rank
//! its files against a query with nucleo. The walk (`FileIndex::build`) is
//! separated from the matcher so the index can be built off the UI thread and
//! shared (`Arc`) by cmd-p and cmd-click resolution. Partial snapshots stream
//! during the walk so results appear before the tree is fully scanned.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

mod walk;

/// Cap on results returned to the UI for a single query.
const MAX_RESULTS: usize = 200;

/// A ranked entry, its path relative to the index's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub path: PathBuf,
    pub is_dir: bool,
    pub score: u32,
}

/// A matchable entry; nucleo ranks against its relative path.
#[derive(Clone)]
struct Entry {
    path: String,
    is_dir: bool,
}

impl AsRef<str> for Entry {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

/// The walked file/directory list for one root. Immutable once built, so it can
/// be wrapped in an `Arc` and shared across finders and threads. `Clone` is for
/// progressive snapshots during a long walk.
#[derive(Clone)]
pub struct FileIndex {
    entries: Vec<Entry>,
}

impl FileIndex {
    /// Walk `root` to completion. Prefer [`build_with_progress`] when the UI
    /// should show partial results during a long walk.
    pub fn build(root: &Path) -> FileIndex {
        FileIndex {
            entries: walk::walk_with_progress(root, |_| true),
        }
    }

    /// Walk `root`, invoking `on_partial` with snapshots as entries accumulate.
    /// Return `false` from the callback to cancel. The final index is returned
    /// (and is also passed to `on_partial` as the last call).
    pub fn build_with_progress(
        root: &Path,
        mut on_partial: impl FnMut(&FileIndex) -> bool,
    ) -> FileIndex {
        let entries = walk::walk_with_progress(root, |slice| {
            on_partial(&FileIndex {
                entries: slice.to_vec(),
            })
        });
        FileIndex { entries }
    }

    /// Number of indexed paths (files + directories).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Rank entries against `query`. Safe to call from a background thread.
    /// An empty query lists paths unranked (path order), capped at `MAX_RESULTS`.
    pub fn query(&self, query: &str) -> Vec<FileMatch> {
        if query.is_empty() {
            return self
                .entries
                .iter()
                .take(MAX_RESULTS)
                .map(|entry| FileMatch {
                    path: PathBuf::from(&entry.path),
                    is_dir: entry.is_dir,
                    score: 0,
                })
                .collect();
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut matches: Vec<FileMatch> = pattern
            .match_list(self.entries.iter(), &mut matcher)
            .into_iter()
            .map(|(entry, score)| FileMatch {
                path: PathBuf::from(&entry.path),
                is_dir: entry.is_dir,
                score,
            })
            .collect();
        matches.sort_by(|a, b| compare_matches(a, b, query));
        matches.truncate(MAX_RESULTS);
        matches
    }
}

pub struct Finder {
    index: Arc<FileIndex>,
}

impl Finder {
    /// A finder over an already-built (possibly shared) index.
    pub fn new(index: Arc<FileIndex>) -> Finder {
        Finder { index }
    }

    /// Rank entries against `query`. An empty query lists them unranked.
    pub fn query(&mut self, query: &str) -> Vec<FileMatch> {
        self.index.query(query)
    }
}

/// Basename match quality for ranking (lower = better).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum BasenameRank {
    Exact = 0,
    Prefix = 1,
    Contains = 2,
    Other = 3,
}

impl BasenameRank {
    fn of(path: &Path, query: &str) -> Self {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return Self::Other;
        };
        let name_l = name.to_lowercase();
        let query_l = query.to_lowercase();
        if name_l == query_l {
            Self::Exact
        } else if name_l.starts_with(&query_l) {
            Self::Prefix
        } else if name_l.contains(&query_l) {
            Self::Contains
        } else {
            Self::Other
        }
    }
}

fn compare_matches(a: &FileMatch, b: &FileMatch, query: &str) -> Ordering {
    let a_path_exact = path_eq_query(&a.path, query);
    let b_path_exact = path_eq_query(&b.path, query);
    b_path_exact
        .cmp(&a_path_exact)
        .then_with(|| BasenameRank::of(&a.path, query).cmp(&BasenameRank::of(&b.path, query)))
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| path_components(&a.path).cmp(&path_components(&b.path)))
        .then_with(|| a.path.as_os_str().len().cmp(&b.path.as_os_str().len()))
        .then_with(|| a.path.cmp(&b.path))
}

fn path_eq_query(path: &Path, query: &str) -> bool {
    path.as_os_str() == query || path.to_string_lossy().eq_ignore_ascii_case(query)
}

fn path_components(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests;
