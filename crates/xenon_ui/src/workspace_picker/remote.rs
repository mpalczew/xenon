//! What a host has told us about the current `ssh://host/…` query, and the
//! rows built from it. Pure state and row-merging rules.

use std::collections::HashSet;
use std::path::PathBuf;

use xenon_ssh::{HostQuery, RemoteDir, tilde_path};

use super::WorkspaceCandidate;
use super::ssh_input::absolute;

/// Answers for one query: folders listed under the parent, and name matches.
#[derive(Clone, Debug)]
pub(crate) struct PathListing {
    pub(super) home: String,
    pub(super) listed: Vec<RemoteDir>,
    pub(super) found: Vec<RemoteDir>,
}

#[derive(Clone, Debug)]
pub(crate) enum Remote {
    /// No request is outstanding.
    Idle,
    Searching,
    Failed(String),
    Ready(PathListing),
}

impl Remote {
    pub(super) fn home(&self) -> &str {
        match self {
            Self::Ready(listing) => &listing.home,
            _ => "",
        }
    }
}

#[cfg(feature = "visual-tests")]
impl Remote {
    pub(crate) fn searching() -> Self {
        Self::Searching
    }

    pub(crate) fn failed(error: &str) -> Self {
        Self::Failed(error.into())
    }

    pub(crate) fn ready(home: &str, listed: &[(&str, bool)], found: &[(&str, bool)]) -> Self {
        let dirs = |rows: &[(&str, bool)]| {
            rows.iter()
                .map(|(path, git)| RemoteDir {
                    path: (*path).into(),
                    git: *git,
                })
                .collect()
        };
        Self::Ready(PathListing {
            home: home.into(),
            listed: dirs(listed),
            found: dirs(found),
        })
    }
}

/// Ask the host for the listing, then (if given) the name search.
pub(super) fn lookup_path(
    host: &str,
    listing: &HostQuery,
    search: Option<&HostQuery>,
) -> anyhow::Result<PathListing> {
    let listed = xenon_ssh::lookup_host(host, listing)?;
    let found = match search {
        Some(query) => xenon_ssh::lookup_host(host, query)?.dirs,
        None => Vec::new(),
    };
    Ok(PathListing {
        home: listed.home,
        listed: listed.dirs,
        found,
    })
}

/// Hosts to offer in the picker: config aliases plus hosts of past workspaces,
/// most recently used first, then config order.
pub(crate) fn host_candidates(
    config: Vec<String>,
    used: &[(String, u64)],
) -> Vec<WorkspaceCandidate> {
    let mut hosts: Vec<(String, u64)> = Vec::new();
    for (name, last) in used {
        match hosts.iter_mut().find(|(n, _)| n == name) {
            Some((_, seen)) => *seen = (*seen).max(*last),
            None => hosts.push((name.clone(), *last)),
        }
    }
    for name in config {
        if !hosts.iter().any(|(n, _)| *n == name) {
            hosts.push((name, 0));
        }
    }
    hosts
        .into_iter()
        .map(|(name, last_opened)| WorkspaceCandidate::Host {
            root: PathBuf::from(format!("ssh://{name}")),
            name,
            last_opened,
        })
        .collect()
}

/// Past workspaces first, then listed folders, then name matches; each folder
/// once. With a listing, a past workspace stays only if the host returned it.
pub(super) fn merge_path(
    host: &str,
    mut recents: Vec<(WorkspaceCandidate, String)>,
    listing: Option<&PathListing>,
) -> Vec<WorkspaceCandidate> {
    let Some(listing) = listing else {
        return recents.into_iter().map(|(c, _)| c).collect();
    };
    let returned: HashSet<&str> = listing
        .listed
        .iter()
        .chain(&listing.found)
        .map(|d| d.path.as_str())
        .collect();
    recents.retain(|(_, dir)| returned.contains(absolute(dir, &listing.home).as_str()));
    let mut seen: HashSet<String> = recents
        .iter()
        .map(|(_, dir)| absolute(dir, &listing.home))
        .collect();
    let mut out: Vec<WorkspaceCandidate> = recents.into_iter().map(|(c, _)| c).collect();
    for dir in listing.listed.iter().chain(&listing.found) {
        if seen.insert(dir.path.clone()) {
            out.push(WorkspaceCandidate::Remote {
                host: host.to_string(),
                root: PathBuf::from(&dir.path),
                display: tilde_path(&dir.path, &listing.home),
            });
        }
    }
    out
}

/// One line of an SSH failure, without the `SSH:` prefix and bounded in length.
pub(super) fn short_error(error: &str) -> String {
    let line = error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("connection failed");
    let line = line.strip_prefix("SSH:").unwrap_or(line).trim();
    match line.char_indices().nth(90) {
        Some((end, _)) => format!("{}…", &line[..end]),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(paths: &[&str]) -> Vec<RemoteDir> {
        paths
            .iter()
            .map(|p| RemoteDir {
                path: (*p).into(),
                git: false,
            })
            .collect()
    }

    fn listing(listed: &[&str], found: &[&str]) -> PathListing {
        PathListing {
            home: "/home/me".into(),
            listed: dirs(listed),
            found: dirs(found),
        }
    }

    fn recent(dir: &str) -> (WorkspaceCandidate, String) {
        let candidate = WorkspaceCandidate::Path {
            root: PathBuf::from(dir),
            found: true,
        };
        (candidate, dir.into())
    }

    fn display(row: &WorkspaceCandidate) -> &str {
        match row {
            WorkspaceCandidate::Remote { display, .. } => display,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn hosts_merge_config_with_used_most_recent_first() {
        let rows = host_candidates(
            vec!["nuc".into(), "thinkpad".into()],
            &[
                ("thinkpad".into(), 5),
                ("devbox".into(), 9),
                ("thinkpad".into(), 7),
            ],
        );
        let names: Vec<_> = rows.iter().map(|r| (r.name(), r.last_opened())).collect();
        assert_eq!(
            names,
            [
                ("ssh://thinkpad/".into(), 7),
                ("ssh://devbox/".into(), 9),
                ("ssh://nuc/".into(), 0)
            ]
        );
        assert!(
            rows.iter()
                .all(|r| r.badge() == Some("SSH") && r.selectable())
        );
    }

    #[test]
    fn listed_folders_come_before_name_matches_without_duplicates() {
        let found = listing(
            &["/home/me/src", "/home/me/sre"],
            &["/home/me/src", "/home/me/dev/sr-old"],
        );
        let rows = merge_path("box", Vec::new(), Some(&found));
        let shown: Vec<_> = rows.iter().map(display).collect();
        assert_eq!(shown, ["~/src", "~/sre", "~/dev/sr-old"]);
    }

    #[test]
    fn recents_lead_and_are_not_repeated() {
        let found = listing(&["/home/me/src/a", "/home/me/src/b"], &[]);
        let rows = merge_path("box", vec![recent("/home/me/src/b")], Some(&found));
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], WorkspaceCandidate::Path { .. }));
        assert_eq!(display(&rows[1]), "~/src/a");
    }

    #[test]
    fn recents_saved_with_a_tilde_match_listed_absolute_paths() {
        let found = listing(&["/home/me/src/a"], &[]);
        let rows = merge_path("box", vec![recent("~/src/a")], Some(&found));
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn recents_the_host_did_not_return_are_dropped_but_kept_while_waiting() {
        let found = listing(&["/home/me/src/a"], &[]);
        let rows = merge_path("box", vec![recent("/elsewhere/b")], Some(&found));
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], WorkspaceCandidate::Remote { .. }));
        let waiting = merge_path("box", vec![recent("/elsewhere/b")], None);
        assert_eq!(waiting.len(), 1);
    }

    #[test]
    fn errors_are_one_bounded_line() {
        assert_eq!(
            short_error("SSH: Permission denied (publickey).\nmore"),
            "Permission denied (publickey)."
        );
        assert_eq!(short_error("  \n"), "connection failed");
        assert!(short_error(&"x".repeat(200)).chars().count() <= 91);
    }
}
