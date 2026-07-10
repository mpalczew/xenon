//! Fuzzy file finder for cmd-p: walk a directory (respecting .gitignore) into a
//! shareable `FileIndex`, then rank its files against a query with nucleo. The
//! walk (`FileIndex::build`) is separated from the matcher so the index can be
//! built off the UI thread and shared (`Arc`) by cmd-p and cmd-click resolution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

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
/// be wrapped in an `Arc` and shared across finders and threads.
pub struct FileIndex {
    entries: Vec<Entry>,
}

impl FileIndex {
    /// Walk `root`, collecting files and directories. Respects .gitignore and
    /// skips hidden entries (so `.git` and friends stay out of results). This is
    /// the blocking step; run it on a background executor.
    pub fn build(root: &Path) -> FileIndex {
        FileIndex {
            entries: walk(root),
        }
    }
}

pub struct Finder {
    index: Arc<FileIndex>,
    matcher: Matcher,
}

impl Finder {
    /// A finder over an already-built (possibly shared) index.
    pub fn new(index: Arc<FileIndex>) -> Finder {
        Finder {
            index,
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Rank entries against `query`. An empty query lists them unranked.
    pub fn query(&mut self, query: &str) -> Vec<FileMatch> {
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut matches: Vec<FileMatch> = pattern
            .match_list(self.index.entries.iter(), &mut self.matcher)
            .into_iter()
            .map(|(entry, score)| FileMatch {
                path: PathBuf::from(&entry.path),
                is_dir: entry.is_dir,
                score,
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
        matches.truncate(MAX_RESULTS);
        matches
    }
}

fn walk(root: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    for entry in WalkBuilder::new(root).build().flatten() {
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
            continue; // the root directory itself
        }
        entries.push(Entry {
            path: relative.to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
        });
    }
    entries
}

#[cfg(test)]
mod tests;
