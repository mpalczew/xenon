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
    match MatchQuality::of_basename(&c.rank_name(), n) {
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
    fn hosts_rank_below_every_local_match() {
        let host = WorkspaceCandidate::Host {
            name: "thinkpad".into(),
            root: PathBuf::from("ssh://thinkpad"),
            last_opened: 999,
        };
        let mut rows = vec![host, found("/Users/me/src/think"), closed("t", "/tmp/t", 1)];
        sort_candidates(&mut rows, Some("think"));
        assert!(matches!(rows[2], WorkspaceCandidate::Host { .. }));
    }
}
