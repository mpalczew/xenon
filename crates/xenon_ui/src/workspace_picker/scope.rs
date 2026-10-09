//! Where the picker is searching: this Mac, or inside one SSH host.
//!
//! Pure state and row-merging rules, so they can be tested without a window.

use std::path::PathBuf;

use xenon_ssh::{HostQuery, RemoteListing, tilde_path};

use super::WorkspaceCandidate;

/// What the host has told us about the current query.
#[derive(Clone, Debug)]
pub(crate) enum Remote {
    /// Nothing typed yet; no request is made.
    Idle,
    Searching,
    Failed(String),
    Ready(RemoteListing),
}

#[cfg(feature = "visual-tests")]
impl Remote {
    pub(crate) fn searching() -> Self {
        Self::Searching
    }

    pub(crate) fn failed(error: &str) -> Self {
        Self::Failed(error.into())
    }

    pub(crate) fn ready(home: &str, dirs: &[(&str, bool)]) -> Self {
        Self::Ready(RemoteListing {
            home: home.into(),
            dirs: dirs
                .iter()
                .map(|(path, git)| xenon_ssh::RemoteDir {
                    path: (*path).into(),
                    git: *git,
                })
                .collect(),
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct HostScope {
    pub(super) name: String,
    pub(super) remote: Remote,
}

#[derive(Clone, Debug)]
pub(super) enum Scope {
    Local,
    Host(HostScope),
}

impl Scope {
    pub(super) fn enter(name: String) -> Self {
        Self::Host(HostScope {
            name,
            remote: Remote::Idle,
        })
    }

    pub(super) fn host(&self) -> Option<&HostScope> {
        match self {
            Self::Host(host) => Some(host),
            Self::Local => None,
        }
    }

    /// Backspace on an empty field steps out of a host; otherwise it just edits.
    pub(super) fn after_backspace(&self, query_empty: bool) -> Option<Self> {
        (query_empty && self.host().is_some()).then_some(Self::Local)
    }
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

/// Past workspaces first, then remote folders not already among them.
/// A typed path keeps only past workspaces the host actually listed.
pub(super) fn merge_remote(
    host: &str,
    mut recents: Vec<(WorkspaceCandidate, String)>,
    query: &HostQuery,
    listing: Option<&RemoteListing>,
) -> Vec<WorkspaceCandidate> {
    let dirs = listing.map(|l| l.dirs.as_slice()).unwrap_or_default();
    let home = listing.map(|l| l.home.as_str()).unwrap_or_default();
    if matches!(query, HostQuery::Dirs { .. }) {
        recents.retain(|(_, dir)| dirs.iter().any(|d| d.path == *dir));
    }
    let mut out: Vec<WorkspaceCandidate> = recents.iter().map(|(c, _)| c.clone()).collect();
    for dir in dirs
        .iter()
        .filter(|d| !recents.iter().any(|(_, r)| *r == d.path))
    {
        out.push(WorkspaceCandidate::Remote {
            host: host.to_string(),
            root: PathBuf::from(&dir.path),
            display: tilde_path(&dir.path, home),
        });
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

/// Text Tab puts in the field: the folder plus `/`, ready to list its children.
pub(super) fn completion(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xenon_ssh::RemoteDir;

    fn listing(paths: &[&str]) -> RemoteListing {
        RemoteListing {
            home: "/home/me".into(),
            dirs: paths
                .iter()
                .map(|p| RemoteDir {
                    path: (*p).into(),
                    git: false,
                })
                .collect(),
        }
    }

    fn recent(dir: &str) -> (WorkspaceCandidate, String) {
        let candidate = WorkspaceCandidate::Path {
            root: PathBuf::from(dir),
            found: true,
        };
        (candidate, dir.into())
    }

    #[test]
    fn entering_a_host_starts_idle_and_backspace_on_empty_leaves() {
        let scope = Scope::enter("thinkpad".into());
        assert!(matches!(scope.host().unwrap().remote, Remote::Idle));
        assert!(scope.after_backspace(false).is_none());
        assert!(matches!(scope.after_backspace(true), Some(Scope::Local)));
        assert!(Scope::Local.after_backspace(true).is_none());
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
                ("thinkpad".into(), 7),
                ("devbox".into(), 9),
                ("nuc".into(), 0)
            ]
        );
        assert!(rows.iter().all(|r| r.badge() == "SSH" && r.selectable()));
    }

    #[test]
    fn name_search_lists_recents_then_new_folders_without_duplicates() {
        let found = listing(&["/home/me/src/xenon", "/home/me/src/xen-old"]);
        let rows = merge_remote(
            "box",
            vec![recent("/home/me/src/xenon")],
            &HostQuery::parse("xen"),
            Some(&found),
        );
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], WorkspaceCandidate::Path { .. }));
        match &rows[1] {
            WorkspaceCandidate::Remote { display, host, .. } => {
                assert_eq!((display.as_str(), host.as_str()), ("~/src/xen-old", "box"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn typed_paths_keep_only_recents_the_host_listed() {
        let found = listing(&["/home/me/src/a"]);
        let rows = merge_remote(
            "box",
            vec![recent("/home/me/src/a"), recent("/elsewhere/b")],
            &HostQuery::parse("~/src/"),
            Some(&found),
        );
        assert_eq!(rows.len(), 1);
        let empty = merge_remote("box", vec![recent("/elsewhere/b")], &HostQuery::Empty, None);
        assert_eq!(empty.len(), 1);
    }

    #[test]
    fn errors_are_one_bounded_line_and_completion_ends_in_slash() {
        assert_eq!(
            short_error("SSH: Permission denied (publickey).\nmore"),
            "Permission denied (publickey)."
        );
        assert_eq!(short_error("  \n"), "connection failed");
        assert!(short_error(&"x".repeat(200)).chars().count() <= 91);
        assert_eq!(completion("~/src/xenon"), "~/src/xenon/");
        assert_eq!(completion("/etc/"), "/etc/");
    }
}
