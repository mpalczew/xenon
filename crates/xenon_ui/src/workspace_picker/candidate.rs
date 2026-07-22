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
}

impl WorkspaceCandidate {
    pub fn name(&self) -> String {
        match self {
            Self::Open { name, .. } | Self::Closed { name, .. } => name.clone(),
            Self::Path { root, .. } => root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        }
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::Open { root, .. } | Self::Closed { root, .. } | Self::Path { root, .. } => root,
        }
    }

    pub fn selectable(&self) -> bool {
        match self {
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
        format!("{} {}", self.name(), self.root().display())
    }

    pub(super) fn badge(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Closed { missing: true, .. } => "missing",
            Self::Closed { .. } => "closed",
            Self::Path { found: true, .. } => "found",
            Self::Path { .. } => "path",
        }
    }

    /// Lower is better. Known workspaces always beat discovery.
    pub(super) fn rank_tier(&self) -> u8 {
        match self {
            // User typed an existing path (or ~ expansion).
            Self::Path { found: false, .. } => 0,
            // Open or closed history — prefer these over FS discovery.
            Self::Open { .. } | Self::Closed { missing: false, .. } => 1,
            Self::Path { found: true, .. } => 2,
            Self::Closed { missing: true, .. } => 3,
        }
    }

    pub(super) fn last_opened(&self) -> u64 {
        match self {
            Self::Open { last_opened, .. } | Self::Closed { last_opened, .. } => *last_opened,
            Self::Path { .. } => 0,
        }
    }

    pub(super) fn workspace_id(&self) -> Option<WorkspaceId> {
        match self {
            Self::Open { id, .. } | Self::Closed { id, .. } => Some(*id),
            Self::Path { .. } => None,
        }
    }

    pub(super) fn is_closed(&self) -> bool {
        matches!(self, Self::Closed { .. })
    }
}

pub enum WorkspacePickerEvent {
    Open(WorkspaceCandidate),
    /// Drop a closed workspace from history (session file removed).
    Forget(WorkspaceId),
    Browse,
    Dismissed,
}
