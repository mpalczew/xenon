//! Where a CLI or agent file open lands.

use std::path::{Path, PathBuf};

use crate::{Registry, WorkspaceId};

impl Registry {
    /// Longest open workspace root that is a prefix of `path`.
    pub(crate) fn workspace_for_path(&self, path: &Path) -> Option<WorkspaceId> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut best: Option<(usize, WorkspaceId)> = None;
        for rec in &self.workspaces {
            let root = rec.root.canonicalize().unwrap_or_else(|_| rec.root.clone());
            if path.starts_with(&root) {
                let len = root.as_os_str().len();
                if best.is_none_or(|(best_len, _)| len > best_len) {
                    best = Some((len, rec.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Pick where a CLI/agent file open lands. The longest open workspace
    /// root containing the file wins. Otherwise the file opens in the current
    /// workspace, even when it lies outside every root (`/tmp`, `~/.cache`).
    /// Closed workspaces never reopen for a file: a closed `~` would otherwise
    /// claim every file under home. With no current workspace, the file's
    /// folder becomes one.
    pub fn route_file(&self, path: &Path, current: Option<WorkspaceId>) -> FileRoute {
        if let Some(id) = self.workspace_for_path(path) {
            FileRoute::Workspace(id)
        } else if current.is_none()
            && let Some(parent) = path.parent()
        {
            FileRoute::NewWorkspace(parent.to_path_buf())
        } else {
            FileRoute::Current
        }
    }
}

/// Where a CLI/agent file open goes; see [`Registry::route_file`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileRoute {
    /// An open workspace whose root contains the file: focus it.
    Workspace(WorkspaceId),
    /// Stay in the current workspace.
    Current,
    /// No workspace at all: register this folder as one.
    NewWorkspace(PathBuf),
}
