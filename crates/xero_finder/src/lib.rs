//! Fuzzy file finder for cmd-p: walk a directory (respecting .gitignore) and
//! rank the files against a query with nucleo.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

/// Cap on results returned to the UI for a single query.
const MAX_RESULTS: usize = 200;

/// A ranked file, its path relative to the finder's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub path: PathBuf,
    pub score: u32,
}

pub struct Finder {
    /// Relative paths as strings, the form nucleo matches against.
    files: Vec<String>,
    matcher: Matcher,
}

impl Finder {
    /// Walk `root` now, collecting matchable files. Respects .gitignore and
    /// skips hidden entries (so `.git` and friends stay out of results).
    pub fn start(root: &Path) -> Finder {
        Finder { files: walk(root), matcher: Matcher::new(Config::DEFAULT) }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Rank files against `query`. An empty query lists files unranked.
    pub fn query(&mut self, query: &str) -> Vec<FileMatch> {
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut matches: Vec<FileMatch> = pattern
            .match_list(self.files.iter(), &mut self.matcher)
            .into_iter()
            .map(|(path, score)| FileMatch { path: PathBuf::from(path), score })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
        matches.truncate(MAX_RESULTS);
        matches
    }
}

fn walk(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for entry in WalkBuilder::new(root).build().flatten() {
        if entry.file_type().is_some_and(|t| t.is_file())
            && let Ok(relative) = entry.path().strip_prefix(root)
        {
            files.push(relative.to_string_lossy().into_owned());
        }
    }
    files
}

#[cfg(test)]
mod tests;
