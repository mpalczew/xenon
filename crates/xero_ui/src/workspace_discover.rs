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

/// Expand `~` / `~/…` and absolute paths.
pub(crate) fn expand_user_path(input: &str) -> Option<PathBuf> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s == "~" {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest));
    }
    if s.starts_with('/') {
        return Some(PathBuf::from(s));
    }
    None
}

/// Canonical existing directory, or None.
pub(crate) fn resolve_existing_dir(path: &Path) -> Option<PathBuf> {
    let path = if path.starts_with("~") {
        expand_user_path(&path.to_string_lossy())?
    } else {
        path.to_path_buf()
    };
    let canon = path.canonicalize().ok()?;
    canon.is_dir().then_some(canon)
}

/// Fast dir check for UI (no canonicalize). Prefer this on the main thread.
pub(crate) fn path_is_dir(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    // `~` or non-normalized paths.
    resolve_existing_dir(path).is_some()
}

pub(crate) fn same_root(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => false,
    }
}

/// Default roots for name-only discovery (only those that exist).
pub(crate) fn default_search_roots() -> Vec<PathBuf> {
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
pub(crate) enum DiscoverQuery {
    /// Exact existing directory from typed path.
    Exact(PathBuf),
    /// `~/src personalfiles` → search under root for needle.
    Scoped { root: PathBuf, needle: String },
    /// Bare name: search under default roots.
    Name(String),
}

pub(crate) fn parse_discover_query(raw: &str) -> Option<DiscoverQuery> {
    let q = raw.trim();
    if q.is_empty() {
        return None;
    }
    // Whole query is an existing path.
    if let Some(root) = expand_user_path(q).and_then(|p| resolve_existing_dir(&p)) {
        return Some(DiscoverQuery::Exact(root));
    }
    // First token path-like + rest = needle.
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
    // Bare name (no leading / or ~) or unresolved path fragment.
    if expand_user_path(q).is_none() && !q.starts_with('/') {
        return Some(DiscoverQuery::Name(q.to_string()));
    }
    None
}

/// Basename match quality for a discovery needle (lower = better).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum MatchQuality {
    /// Basename equals needle (case-insensitive).
    Exact = 0,
    /// Basename starts with needle.
    Prefix = 1,
    /// Basename contains needle elsewhere.
    Contains = 2,
}

impl MatchQuality {
    pub(crate) fn of_basename(name: &str, needle: &str) -> Option<Self> {
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
pub(crate) struct FoundRoot {
    pub path: PathBuf,
    pub git: bool,
    pub quality: MatchQuality,
}

/// On-demand directory discovery (depth + visit capped). Safe for background thread.
pub(crate) fn discover(query: &DiscoverQuery) -> Vec<FoundRoot> {
    match query {
        DiscoverQuery::Exact(path) => vec![FoundRoot {
            git: path.join(".git").exists(),
            path: path.clone(),
            quality: MatchQuality::Exact,
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
        .filter(|f| f.quality == MatchQuality::Exact)
        .count()
}

fn finish_found(out: &mut Vec<FoundRoot>) {
    sort_found(out);
    out.truncate(MAX_RESULTS);
}

fn sort_found(out: &mut [FoundRoot]) {
    out.sort_by(|a, b| {
        a.quality
            .cmp(&b.quality)
            .then_with(|| path_depth(&a.path).cmp(&path_depth(&b.path)))
            .then_with(|| a.path.as_os_str().len().cmp(&b.path.as_os_str().len()))
            .then_with(|| b.git.cmp(&a.git))
            .then_with(|| a.path.cmp(&b.path))
    });
}

fn path_depth(path: &Path) -> usize {
    path.components().count()
}

/// Breadth-first so shallow complete matches beat deep noise under visit caps.
fn walk_for_name(root: &Path, needle: &str, out: &mut Vec<FoundRoot>) {
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut qi = 0;
    let mut visits = 0usize;
    let mut seen: HashSet<PathBuf> = HashSet::new();

    while qi < queue.len() {
        if visits >= MAX_VISITS || exact_count(out) >= MAX_RESULTS {
            break;
        }
        let (dir, depth) = queue[qi].clone();
        qi += 1;
        if depth > MAX_DEPTH {
            continue;
        }
        visits += 1;
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
                push_found(out, &mut seen, path.clone(), quality);
            }
            if depth < MAX_DEPTH {
                queue.push((path, depth + 1));
            }
        }
    }
}

fn push_found(
    out: &mut Vec<FoundRoot>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
    quality: MatchQuality,
) {
    if !seen.insert(path.clone()) {
        return;
    }
    let candidate = FoundRoot {
        git: path.join(".git").exists(),
        path,
        quality,
    };
    if out.len() < MAX_COLLECT {
        out.push(candidate);
        return;
    }
    // Cap full: still accept a better match by replacing a weaker one.
    if let Some((i, _)) = out
        .iter()
        .enumerate()
        .filter(|(_, f)| f.quality > candidate.quality)
        .max_by_key(|(_, f)| f.quality)
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
            // Large content trees that are never workspace roots.
            | "shows"
            | "Library"
            | "Applications"
    ) || name.starts_with('.')
}

/// Needle used for basename ranking in the picker (scoped rest or bare name).
pub(crate) fn ranking_needle(raw: &str) -> Option<String> {
    match parse_discover_query(raw)? {
        DiscoverQuery::Scoped { needle, .. } | DiscoverQuery::Name(needle) => Some(needle),
        DiscoverQuery::Exact(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_tree() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        // Noise: deep contains match
        let deep = root.join("aaa/bbb/ccc/myskills-helper");
        fs::create_dir_all(&deep).unwrap();
        // Prefix match at shallow depth
        let prefix = root.join("skills-lab");
        fs::create_dir_all(&prefix).unwrap();
        // Exact match, short path, with .git — should win
        let exact = root.join("agentic/skills");
        fs::create_dir_all(&exact).unwrap();
        fs::create_dir_all(exact.join(".git")).unwrap();
        // Exact match deeper
        let deep_exact = root.join("vendorish/nested/skills");
        fs::create_dir_all(&deep_exact).unwrap();
        (tmp, root)
    }

    #[test]
    fn match_quality_prefers_exact() {
        assert_eq!(
            MatchQuality::of_basename("skills", "skills"),
            Some(MatchQuality::Exact)
        );
        assert_eq!(
            MatchQuality::of_basename("skills-lab", "skills"),
            Some(MatchQuality::Prefix)
        );
        assert_eq!(
            MatchQuality::of_basename("myskills", "skills"),
            Some(MatchQuality::Contains)
        );
        assert_eq!(MatchQuality::of_basename("other", "skills"), None);
    }

    #[test]
    fn sort_prefers_exact_then_short_path() {
        let (_tmp, root) = tmp_tree();
        let mut out = Vec::new();
        walk_for_name(&root, "skills", &mut out);
        finish_found(&mut out);
        assert!(!out.is_empty());
        let top = &out[0];
        assert_eq!(top.quality, MatchQuality::Exact);
        assert!(
            top.path.ends_with("agentic/skills"),
            "top was {:?}, expected agentic/skills",
            top.path
        );
        let first_non_exact = out.iter().position(|f| f.quality != MatchQuality::Exact);
        if let Some(i) = first_non_exact {
            assert!(out[..i].iter().all(|f| f.quality == MatchQuality::Exact));
        }
    }

    #[test]
    fn visit_cap_stops() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // Many siblings so BFS would be large without a cap.
        for i in 0..100 {
            fs::create_dir_all(root.join(format!("p{i}/skills"))).unwrap();
        }
        let mut out = Vec::new();
        walk_for_name(root, "skills", &mut out);
        // With many exact matches we stop early; must not hang / collect unbounded.
        assert!(out.len() <= MAX_COLLECT);
        assert!(exact_count(&out) >= 1);
    }
}
