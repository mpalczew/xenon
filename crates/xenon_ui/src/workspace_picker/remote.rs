//! What a host has told us about the current `ssh://host/…` query, and the
//! rows built from it. Pure state and row-merging rules.

use std::collections::HashSet;
use std::path::PathBuf;

use xenon_ssh::{HostQuery, RemoteDir, RemoteListing, tilde_path};

use super::WorkspaceCandidate;
use super::ssh_input::absolute;

/// Where the slow name search stands once the folder listing has arrived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Discovery {
    Pending,
    Done,
    Failed(String),
}

/// Answers for one query: folders listed under the parent, and name matches.
#[derive(Clone, Debug)]
pub(crate) struct PathListing {
    pub(super) home: String,
    pub(super) listed: Vec<RemoteDir>,
    pub(super) found: Vec<RemoteDir>,
    pub(super) discovery: Discovery,
}

/// One host answer, tagged with the query generation that asked for it.
#[derive(Debug)]
pub(super) struct Reply {
    pub(super) token: u64,
    pub(super) body: ReplyBody,
}

#[derive(Debug)]
pub(super) enum ReplyBody {
    /// The folder listing; `searching` says a name search will follow.
    Listed {
        listing: Result<RemoteListing, String>,
        searching: bool,
    },
    Discovered(Result<RemoteListing, String>),
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

    /// Something is still on its way: rows may yet be added.
    pub(super) fn waiting(&self) -> bool {
        match self {
            Self::Searching | Self::Failed(_) => true,
            Self::Ready(listing) => listing.discovery != Discovery::Done,
            Self::Idle => false,
        }
    }

    /// Every answer is in, so the host's rows are the complete picture.
    pub(super) fn complete(&self) -> bool {
        matches!(self, Self::Ready(l) if l.discovery == Discovery::Done)
    }

    /// Fold a reply in. A reply from an older query (`token != current`), or a
    /// discovery with no listing to attach to, changes nothing.
    pub(super) fn apply(self, current: u64, reply: Reply) -> Self {
        if reply.token != current {
            return self;
        }
        match (reply.body, self) {
            (ReplyBody::Listed { listing, searching }, _) => match listing {
                Ok(l) => Self::Ready(PathListing {
                    home: l.home,
                    listed: l.dirs,
                    found: Vec::new(),
                    discovery: if searching {
                        Discovery::Pending
                    } else {
                        Discovery::Done
                    },
                }),
                Err(error) => Self::Failed(error),
            },
            (ReplyBody::Discovered(result), Self::Ready(mut listing)) => {
                match result {
                    Ok(found) => {
                        listing.found = found.dirs;
                        listing.discovery = Discovery::Done;
                    }
                    Err(error) => listing.discovery = Discovery::Failed(error),
                }
                Self::Ready(listing)
            }
            (ReplyBody::Discovered(_), other) => other,
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
            discovery: Discovery::Done,
        })
    }

    /// Listing on screen, name search still running.
    pub(crate) fn discovering(home: &str, listed: &[(&str, bool)]) -> Self {
        match Self::ready(home, listed, &[]) {
            Self::Ready(listing) => Self::Ready(PathListing {
                discovery: Discovery::Pending,
                ..listing
            }),
            other => other,
        }
    }
}

/// Run one host lookup, reporting failure as a single short line.
pub(super) fn lookup_line(host: &str, query: &HostQuery) -> Result<RemoteListing, String> {
    xenon_ssh::lookup_host(host, query).map_err(|e| short_error(&e.to_string()))
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
/// once. Once every answer is in, a past workspace stays only if the host
/// returned it.
pub(super) fn merge_path(
    host: &str,
    mut recents: Vec<(WorkspaceCandidate, String)>,
    listing: Option<&PathListing>,
) -> Vec<WorkspaceCandidate> {
    let Some(listing) = listing else {
        return recents.into_iter().map(|(c, _)| c).collect();
    };
    if listing.discovery == Discovery::Done {
        let returned: HashSet<&str> = listing
            .listed
            .iter()
            .chain(&listing.found)
            .map(|d| d.path.as_str())
            .collect();
        recents.retain(|(_, dir)| returned.contains(absolute(dir, &listing.home).as_str()));
    }
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
mod tests;
