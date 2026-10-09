//! Palette rows for open / closed history / discovered paths.

use std::path::{Path, PathBuf};

use xenon_core::WorkspaceId;

use crate::workspace_discover::path_is_dir;

/// One row the user can confirm (or see as missing).
#[derive(Clone, Debug)]
pub enum WorkspaceCandidate {
    Open {
        id: WorkspaceId,
        name: String,
        root: PathBuf,
        /// Unix seconds; 0 if never recorded (legacy).
        last_opened: u64,
    },
    Closed {
        id: WorkspaceId,
        name: String,
        root: PathBuf,
        missing: bool,
        last_opened: u64,
    },
    /// Typed or discovered existing directory.
    Path { root: PathBuf, found: bool },
    /// An SSH host from `~/.ssh/config` or a past SSH workspace; Enter steps inside.
    Host {
        name: String,
        root: PathBuf,
        last_opened: u64,
    },
    /// A folder found on the host the picker is inside; Enter opens it over SSH.
    Remote {
        host: String,
        /// Absolute remote path.
        root: PathBuf,
        /// Same path, `~`-shortened for display.
        display: String,
    },
}

impl WorkspaceCandidate {
    pub fn name(&self) -> String {
        match self {
            Self::Open { name, .. } | Self::Closed { name, .. } | Self::Host { name, .. } => {
                name.clone()
            }
            Self::Path { root, .. } | Self::Remote { root, .. } => root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        }
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::Open { root, .. }
            | Self::Closed { root, .. }
            | Self::Path { root, .. }
            | Self::Host { root, .. }
            | Self::Remote { root, .. } => root,
        }
    }

    pub fn selectable(&self) -> bool {
        if self.root().starts_with("/__xenon_ssh__") {
            return true;
        }
        match self {
            Self::Host { .. } | Self::Remote { .. } => true,
            Self::Closed { missing: true, .. } => false,
            Self::Open { root, .. }
            | Self::Closed {
                root,
                missing: false,
                ..
            } => path_is_dir(root),
            Self::Path { root, .. } => path_is_dir(root),
        }
    }

    pub(super) fn haystack(&self) -> String {
        match self {
            Self::Host { name, .. } => name.clone(),
            _ => format!("{} {}", self.name(), self.root().display()),
        }
    }

    pub(super) fn badge(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Closed { missing: true, .. } => "missing",
            Self::Closed { .. } => "closed",
            Self::Path { found: true, .. } | Self::Remote { .. } => "found",
            Self::Host { .. } => "SSH",
            Self::Path { .. } => "path",
        }
    }

    /// Lower is better. Known workspaces always beat discovery.
    pub(super) fn rank_tier(&self) -> u8 {
        match self {
            // User typed an existing path (or ~ expansion).
            Self::Path { found: false, .. } => 0,
            // Open or closed history — prefer these over FS discovery.
            Self::Open { .. } | Self::Closed { missing: false, .. } | Self::Host { .. } => 1,
            Self::Path { found: true, .. } | Self::Remote { .. } => 2,
            Self::Closed { missing: true, .. } => 3,
        }
    }

    pub(super) fn last_opened(&self) -> u64 {
        match self {
            Self::Open { last_opened, .. }
            | Self::Closed { last_opened, .. }
            | Self::Host { last_opened, .. } => *last_opened,
            Self::Path { .. } | Self::Remote { .. } => 0,
        }
    }

    pub(super) fn workspace_id(&self) -> Option<WorkspaceId> {
        match self {
            Self::Open { id, .. } | Self::Closed { id, .. } => Some(*id),
            Self::Path { .. } | Self::Host { .. } | Self::Remote { .. } => None,
        }
    }

    /// Row subtitle for candidates that are not known SSH workspaces.
    pub(super) fn subtitle(&self) -> Option<String> {
        match self {
            Self::Host { .. } => None,
            Self::Remote { display, .. } => Some(display.clone()),
            _ => Some(self.root().display().to_string()),
        }
    }

    /// Re-point a local candidate at its canonical directory.
    pub(super) fn with_root(self, root: PathBuf) -> Self {
        match self {
            Self::Open {
                id,
                name,
                last_opened,
                ..
            } => Self::Open {
                id,
                name,
                root,
                last_opened,
            },
            Self::Closed {
                id,
                name,
                last_opened,
                ..
            } => Self::Closed {
                id,
                name,
                root,
                missing: false,
                last_opened,
            },
            Self::Path { found, .. } => Self::Path { root, found },
            other => other,
        }
    }

    pub(super) fn is_closed(&self) -> bool {
        matches!(self, Self::Closed { .. })
    }
}

pub enum WorkspacePickerEvent {
    Ssh(String),
    Open(WorkspaceCandidate),
    /// Drop a closed workspace from history (session file removed).
    Forget(WorkspaceId),
    Browse,
    Dismissed,
}
