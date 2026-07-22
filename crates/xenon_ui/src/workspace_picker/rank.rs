//! Sort known workspaces ahead of discovery; basename then MRU within a tier.

use std::cmp::Ordering;
use std::path::Path;

use crate::workspace_discover::MatchQuality;

use super::WorkspaceCandidate;

/// Sort known-first, then basename quality, then MRU (newest first).
pub(super) fn sort_candidates(results: &mut [WorkspaceCandidate], needle: Option<&str>) {
    results.sort_by(|a, b| cmp_candidates(a, b, needle));
}

fn cmp_candidates(
    a: &WorkspaceCandidate,
    b: &WorkspaceCandidate,
    needle: Option<&str>,
) -> Ordering {
    a.rank_tier()
        .cmp(&b.rank_tier())
        .then_with(|| basename_rank(a, needle).cmp(&basename_rank(b, needle)))
        // MRU: higher last_opened first.
        .then_with(|| b.last_opened().cmp(&a.last_opened()))
        .then_with(|| path_component_count(a.root()).cmp(&path_component_count(b.root())))
        .then_with(|| a.root().as_os_str().len().cmp(&b.root().as_os_str().len()))
        .then_with(|| a.name().cmp(&b.name()))
}

/// Lower is better. Exact basename match to the discovery needle ranks first.
fn basename_rank(c: &WorkspaceCandidate, needle: Option<&str>) -> u8 {
    let Some(n) = needle else {
        return 1;
    };
    match MatchQuality::of_basename(&c.name(), n) {
        Some(MatchQuality::Exact) => 0,
        Some(MatchQuality::Prefix) => 1,
        Some(MatchQuality::Contains) => 2,
        None => 3,
    }
}

fn path_component_count(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use xenon_core::WorkspaceId;

    fn closed(name: &str, root: &str, last: u64) -> WorkspaceCandidate {
        WorkspaceCandidate::Closed {
            id: WorkspaceId::new(),
            name: name.into(),
            root: PathBuf::from(root),
            missing: false,
            last_opened: last,
        }
    }

    fn found(root: &str) -> WorkspaceCandidate {
        WorkspaceCandidate::Path {
            root: PathBuf::from(root),
            found: true,
        }
    }

    #[test]
    fn known_beats_discovered_even_when_basename_weaker() {
        // personalfiles (prefix) closed vs exact-named discover "personal"
        let mut rows = vec![
            found("/Users/me/src/other/personal"),
            closed("personalfiles", "/Users/me/src/personalfiles", 100),
        ];
        sort_candidates(&mut rows, Some("personal"));
        assert_eq!(rows[0].name(), "personalfiles");
        assert!(matches!(rows[0], WorkspaceCandidate::Closed { .. }));
        assert!(matches!(
            rows[1],
            WorkspaceCandidate::Path { found: true, .. }
        ));
    }

    #[test]
    fn mru_orders_known_when_basename_equal() {
        let mut rows = vec![
            closed("xenon", "/Users/me/src/old/xenon", 10),
            closed("xenon", "/Users/me/src/new/xenon", 99),
        ];
        sort_candidates(&mut rows, Some("xenon"));
        assert_eq!(rows[0].root(), Path::new("/Users/me/src/new/xenon"));
        assert_eq!(rows[1].root(), Path::new("/Users/me/src/old/xenon"));
    }

    #[test]
    fn empty_query_mru_first() {
        let mut rows = vec![
            closed("a", "/tmp/a", 1),
            closed("b", "/tmp/b", 50),
            closed("c", "/tmp/c", 20),
        ];
        sort_candidates(&mut rows, None);
        assert_eq!(rows[0].name(), "b");
        assert_eq!(rows[1].name(), "c");
        assert_eq!(rows[2].name(), "a");
    }

    #[test]
    fn exact_basename_beats_prefix_among_known() {
        let mut rows = vec![
            closed("personalfiles", "/tmp/personalfiles", 999),
            closed("personal", "/tmp/personal", 1),
        ];
        sort_candidates(&mut rows, Some("personal"));
        assert_eq!(rows[0].name(), "personal");
        assert_eq!(rows[1].name(), "personalfiles");
    }
}
