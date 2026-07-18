//! Fuzzy file finder for cmd-p: walk a directory (respecting .gitignore, including
//! hidden/dot files but not heavy trees) into a shareable `FileIndex`, then rank
//! its files against a query with nucleo. The walk (`FileIndex::build`) is
//! separated from the matcher so the index can be built off the UI thread and
//! shared (`Arc`) by cmd-p and cmd-click resolution. Partial snapshots stream
//! during the walk so results appear before the tree is fully scanned.

use std::cmp::Ordering;
use std::collections::HashMap;
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
    /// Empty query lists paths (recents first, then walk order), capped at
    /// `MAX_RESULTS`. `recents` is most-recent first; among equal match quality
    /// it wins. Pass `&[]` when no MRU is available.
    pub fn query(&self, query: &str, recents: &[PathBuf]) -> Vec<FileMatch> {
        let recent_rank = recent_ranks(recents);
        if query.is_empty() {
            return self.list_empty(&recent_rank);
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
        matches.sort_by(|a, b| compare_matches(a, b, query, &recent_rank));
        matches.truncate(MAX_RESULTS);
        matches
    }

    fn list_empty(&self, recent_rank: &HashMap<PathBuf, u32>) -> Vec<FileMatch> {
        let mut out = Vec::with_capacity(MAX_RESULTS.min(self.entries.len()));
        let mut used: HashMap<&str, ()> = HashMap::new();

        // Recents first (most-recent → oldest), only if still in the index.
        let mut ordered: Vec<(&PathBuf, u32)> = recent_rank.iter().map(|(p, r)| (p, *r)).collect();
        ordered.sort_by_key(|(_, r)| *r);
        for (path, _) in ordered {
            if out.len() >= MAX_RESULTS {
                break;
            }
            let Some(entry) = self
                .entries
                .iter()
                .find(|e| Path::new(&e.path) == path.as_path())
            else {
                continue;
            };
            used.insert(entry.path.as_str(), ());
            out.push(FileMatch {
                path: PathBuf::from(&entry.path),
                is_dir: entry.is_dir,
                score: 0,
            });
        }

        for entry in &self.entries {
            if out.len() >= MAX_RESULTS {
                break;
            }
            if used.contains_key(entry.path.as_str()) {
                continue;
            }
            out.push(FileMatch {
                path: PathBuf::from(&entry.path),
                is_dir: entry.is_dir,
                score: 0,
            });
        }
        out
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

    /// Rank entries against `query`. See [`FileIndex::query`].
    pub fn query(&mut self, query: &str, recents: &[PathBuf]) -> Vec<FileMatch> {
        self.index.query(query, recents)
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

/// Map relative path → rank (0 = most recent). Missing paths rank last.
fn recent_ranks(recents: &[PathBuf]) -> HashMap<PathBuf, u32> {
    let mut ranks = HashMap::with_capacity(recents.len());
    for (i, path) in recents.iter().enumerate() {
        // First occurrence wins (list is most-recent first).
        ranks.entry(path.clone()).or_insert(i as u32);
    }
    ranks
}

fn recent_of(path: &Path, ranks: &HashMap<PathBuf, u32>) -> u32 {
    ranks.get(path).copied().unwrap_or(u32::MAX)
}

fn compare_matches(
    a: &FileMatch,
    b: &FileMatch,
    query: &str,
    recent: &HashMap<PathBuf, u32>,
) -> Ordering {
    let a_path_exact = path_eq_query(&a.path, query);
    let b_path_exact = path_eq_query(&b.path, query);
    b_path_exact
        .cmp(&a_path_exact)
        .then_with(|| BasenameRank::of(&a.path, query).cmp(&BasenameRank::of(&b.path, query)))
        // Among equally good matches, prefer recently opened.
        .then_with(|| recent_of(&a.path, recent).cmp(&recent_of(&b.path, recent)))
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
