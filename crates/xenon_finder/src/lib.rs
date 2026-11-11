use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

pub mod discover;
pub mod fileops;
pub mod fsops;
mod ignored;
mod walk;

pub use ignored::IgnoreMatcher;

const MAX_RESULTS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub path: PathBuf,
    pub is_dir: bool,
    pub score: u32,
    pub hits: Vec<u32>,
}

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

#[derive(Clone)]
pub struct FileIndex {
    entries: Vec<Entry>,
}

impl FileIndex {
    pub fn build(root: &Path) -> FileIndex {
        FileIndex {
            entries: walk::collect(root),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Files whose path ends with `reference` on whole path components, so
    /// `docs/a.md` matches `x/docs/a.md` but not `x/mydocs/a.md`. Leading `./`
    /// and `../` in the reference are ignored. Safe to call from a worker thread.
    pub fn files_ending_with(&self, reference: &str) -> Vec<PathBuf> {
        let wanted: Vec<_> = Path::new(reference)
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(name) => Some(name),
                _ => None,
            })
            .collect();
        if wanted.is_empty() {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| Path::new(&e.path))
            .filter(|p| {
                let have: Vec<_> = p.components().collect();
                have.len() >= wanted.len()
                    && have[have.len() - wanted.len()..]
                        .iter()
                        .zip(&wanted)
                        .all(|(a, b)| a.as_os_str() == *b)
            })
            .map(Path::to_path_buf)
            .collect()
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
        let mut utf32 = Vec::new();
        let mut hit_buf = Vec::new();
        let mut matches: Vec<FileMatch> = pattern
            .match_list(self.entries.iter(), &mut matcher)
            .into_iter()
            .map(|(entry, score)| {
                hit_buf.clear();
                let haystack = Utf32Str::new(&entry.path, &mut utf32);
                pattern.indices(haystack, &mut matcher, &mut hit_buf);
                hit_buf.sort_unstable();
                hit_buf.dedup();
                FileMatch {
                    path: PathBuf::from(&entry.path),
                    is_dir: entry.is_dir,
                    score,
                    hits: hit_buf.clone(),
                }
            })
            .collect();
        matches.sort_by(|a, b| compare_matches(a, b, query, &recent_rank));
        matches.truncate(MAX_RESULTS);
        matches
    }

    fn list_empty(&self, recent_rank: &HashMap<PathBuf, u32>) -> Vec<FileMatch> {
        let mut recent: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|e| recent_rank.contains_key(Path::new(&e.path)))
            .collect();
        recent.sort_by_key(|e| recent_rank[Path::new(&e.path)]);
        let rest = self
            .entries
            .iter()
            .filter(|e| !recent_rank.contains_key(Path::new(&e.path)));
        recent
            .into_iter()
            .chain(rest)
            .take(MAX_RESULTS)
            .map(|e| FileMatch {
                path: PathBuf::from(&e.path),
                is_dir: e.is_dir,
                score: 0,
                hits: Vec::new(),
            })
            .collect()
    }
}

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

fn recent_ranks(recents: &[PathBuf]) -> HashMap<PathBuf, u32> {
    let mut ranks = HashMap::with_capacity(recents.len());
    for (i, path) in recents.iter().enumerate() {
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
