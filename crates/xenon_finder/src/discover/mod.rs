//! Resolve workspace paths and discover project dirs under default / scoped roots.
//!
//! Discovery walks are intentionally bounded: they must be safe to run on a
//! background thread without multi-second freezes on large `~/src` trees.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 3;
const MAX_RESULTS: usize = 20;
/// Soft collect cap; exact matches may still displace weaker ones.
const MAX_COLLECT: usize = 64;
/// Hard stop: never readdir more than this many directories per discover call.
const MAX_VISITS: usize = 2_500;

mod parent;
mod paths;
pub use parent::list_parent_candidates;
pub use paths::{expand_user_path, path_is_dir, resolve_existing_dir, same_root};

/// Default roots for name-only discovery (only those that exist).
fn default_search_roots() -> Vec<PathBuf> {
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => return Vec::new(),
    };
    ["src", "dev", "code", "Projects", "Developer"]
        .into_iter()
        .map(|name| home.join(name))
        .filter(|p| p.is_dir())
        .collect()
}

/// How to interpret the palette query for discovery.
#[derive(Debug, Clone)]
pub enum DiscoverQuery {
    /// Exact existing directory from typed path.
    Exact(PathBuf),
    /// `~/src personalfiles` → search under root for needle.
    Scoped { root: PathBuf, needle: String },
    /// Bare name: search under default roots.
    Name(String),
}

pub fn parse_discover_query(raw: &str) -> Option<DiscoverQuery> {
    let q = raw.trim();
    if q.is_empty() {
        return None;
    }
    if let Some(root) = expand_user_path(q).and_then(|p| resolve_existing_dir(&p)) {
        return Some(DiscoverQuery::Exact(root));
    }
    if let Some((first, rest)) = q.split_once(char::is_whitespace) {
        let rest = rest.trim();
        if !rest.is_empty()
            && let Some(root) = expand_user_path(first).and_then(|p| resolve_existing_dir(&p))
        {
            return Some(DiscoverQuery::Scoped {
                root,
                needle: rest.to_string(),
            });
        }
    }
    if expand_user_path(q).is_none() && !q.starts_with('/') {
        return Some(DiscoverQuery::Name(q.to_string()));
    }
    None
}

/// Basename match quality for a discovery needle (lower = better).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum MatchQuality {
    /// Basename equals needle (case-insensitive).
    Exact = 0,
    /// Basename starts with needle.
    Prefix = 1,
    /// Basename contains needle elsewhere.
    Contains = 2,
}

impl MatchQuality {
    pub fn of_basename(name: &str, needle: &str) -> Option<Self> {
        let name_l = name.to_lowercase();
        let needle_l = needle.to_lowercase();
        if needle_l.is_empty() {
            return None;
        }
        if name_l == needle_l {
            Some(Self::Exact)
        } else if name_l.starts_with(&needle_l) {
            Some(Self::Prefix)
        } else if name_l.contains(&needle_l) {
            Some(Self::Contains)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct FoundRoot {
    pub path: PathBuf,
    pub git: bool,
    pub quality: MatchQuality,
    /// A directory below the search root (not the root itself) is a git repo
    /// above this one. Such folders rank after project roots.
    pub inside_repo: bool,
}

/// On-demand directory discovery (depth + visit capped). Safe for background thread.
pub fn discover(query: &DiscoverQuery) -> Vec<FoundRoot> {
    match query {
        DiscoverQuery::Exact(path) => vec![FoundRoot {
            git: path.join(".git").exists(),
            path: path.clone(),
            quality: MatchQuality::Exact,
            inside_repo: false,
        }],
        DiscoverQuery::Scoped { root, needle } => {
            let mut out = Vec::new();
            walk_for_name(root, needle, &mut out);
            finish_found(&mut out);
            out
        }
        DiscoverQuery::Name(needle) => {
            let mut out = Vec::new();
            for root in default_search_roots() {
                walk_for_name(&root, needle, &mut out);
                if exact_count(&out) >= MAX_RESULTS {
                    break;
                }
            }
            finish_found(&mut out);
            out
        }
    }
}

fn exact_count(out: &[FoundRoot]) -> usize {
    out.iter()
        .filter(|f| f.quality == MatchQuality::Exact && !f.inside_repo)
        .count()
}

fn finish_found(out: &mut Vec<FoundRoot>) {
    sort_found(out);
    out.truncate(MAX_RESULTS);
}

/// Project roots and plain folders before folders inside a repo, then match
/// quality, then shallow, short, git, alphabetical.
fn found_order(a: &FoundRoot, b: &FoundRoot) -> std::cmp::Ordering {
    a.inside_repo
        .cmp(&b.inside_repo)
        .then_with(|| a.quality.cmp(&b.quality))
        .then_with(|| path_depth(&a.path).cmp(&path_depth(&b.path)))
        .then_with(|| a.path.as_os_str().len().cmp(&b.path.as_os_str().len()))
        .then_with(|| b.git.cmp(&a.git))
        .then_with(|| a.path.cmp(&b.path))
}

fn sort_found(out: &mut [FoundRoot]) {
    out.sort_by(found_order);
}

fn path_depth(path: &Path) -> usize {
    path.components().count()
}

/// Breadth-first so shallow complete matches beat deep noise under visit caps.
fn walk_for_name(root: &Path, needle: &str, out: &mut Vec<FoundRoot>) {
    let mut queue: Vec<(PathBuf, usize, bool)> = vec![(root.to_path_buf(), 0, false)];
    let mut qi = 0;
    let mut visits = 0usize;
    let mut seen: HashSet<PathBuf> = HashSet::new();

    while qi < queue.len() {
        if visits >= MAX_VISITS || exact_count(out) >= MAX_RESULTS {
            break;
        }
        let (dir, depth, inside) = queue[qi].clone();
        qi += 1;
        if depth > MAX_DEPTH {
            continue;
        }
        visits += 1;
        let inside = inside || (depth > 0 && dir.join(".git").exists());
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if should_skip_dir(&name) {
                continue;
            }
            let path = entry.path();
            if let Some(quality) = MatchQuality::of_basename(&name, needle) {
                push_found(out, &mut seen, path.clone(), quality, inside);
            }
            if depth < MAX_DEPTH {
                queue.push((path, depth + 1, inside));
            }
        }
    }
}

fn push_found(
    out: &mut Vec<FoundRoot>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
    quality: MatchQuality,
    inside_repo: bool,
) {
    if !seen.insert(path.clone()) {
        return;
    }
    let candidate = FoundRoot {
        git: path.join(".git").exists(),
        path,
        quality,
        inside_repo,
    };
    if out.len() < MAX_COLLECT {
        out.push(candidate);
        return;
    }
    if let Some((i, worst)) = out.iter().enumerate().max_by(|a, b| found_order(a.1, b.1))
        && found_order(&candidate, worst).is_lt()
    {
        out[i] = candidate;
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | ".svn"
            | ".hg"
            | "dist"
            | "build"
            | ".next"
            | ".cache"
            | "vendor"
            | "__pycache__"
            | ".tox"
            | ".venv"
            | "venv"
            | "shows"
            | "Library"
            | "Applications"
    ) || name.starts_with('.')
}

/// Needle used for basename ranking in the picker (scoped rest or bare name).
pub fn ranking_needle(raw: &str) -> Option<String> {
    match parse_discover_query(raw)? {
        DiscoverQuery::Scoped { needle, .. } | DiscoverQuery::Name(needle) => Some(needle),
        DiscoverQuery::Exact(_) => None,
    }
}
