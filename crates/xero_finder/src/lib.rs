//! Fuzzy file finder for cmd-p: walk a directory (respecting .gitignore) and
//! rank the files against a query with nucleo.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

/// Cap on results returned to the UI for a single query.
const MAX_RESULTS: usize = 200;

/// A ranked entry, its path relative to the finder's root.
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

pub struct Finder {
    entries: Vec<Entry>,
    matcher: Matcher,
}

impl Finder {
    /// Walk `root` now, collecting files and directories. Respects .gitignore
    /// and skips hidden entries (so `.git` and friends stay out of results).
    pub fn start(root: &Path) -> Finder {
        Finder { entries: walk(root), matcher: Matcher::new(Config::DEFAULT) }
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Rank entries against `query`. An empty query lists them unranked.
    pub fn query(&mut self, query: &str) -> Vec<FileMatch> {
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut matches: Vec<FileMatch> = pattern
            .match_list(self.entries.iter(), &mut self.matcher)
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
