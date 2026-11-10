use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 3;
const MAX_RESULTS: usize = 20;

mod parent;
mod paths;
pub use parent::list_parent_candidates;
pub use paths::{expand_user_path, path_is_dir, resolve_existing_dir, same_root};

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

#[derive(Debug, Clone)]
pub enum DiscoverQuery {
    Exact(PathBuf),
    Scoped { root: PathBuf, needle: String },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum MatchQuality {
    Exact = 0,
    Prefix = 1,
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
    pub inside_repo: bool,
}

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

fn walk_for_name(root: &Path, needle: &str, out: &mut Vec<FoundRoot>) {
    for path in crate::walk::subdirs(root, MAX_DEPTH + 1) {
        let Some(quality) = path
            .file_name()
            .and_then(|n| MatchQuality::of_basename(&n.to_string_lossy(), needle))
        else {
            continue;
        };
        let inside_repo = path
            .ancestors()
            .skip(1)
            .take_while(|a| *a != root)
            .any(|a| a.join(".git").exists());
        out.push(FoundRoot {
            git: path.join(".git").exists(),
            path,
            quality,
            inside_repo,
        });
    }
}

pub fn ranking_needle(raw: &str) -> Option<String> {
    match parse_discover_query(raw)? {
        DiscoverQuery::Scoped { needle, .. } | DiscoverQuery::Name(needle) => Some(needle),
        DiscoverQuery::Exact(_) => None,
    }
}
