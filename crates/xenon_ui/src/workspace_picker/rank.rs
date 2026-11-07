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
