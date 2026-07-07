//! Workspaces and the persisted registry that indexes them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::{StreamId, WorkspaceId};

/// A registered project folder and the ids of its streams. Session details for
/// each stream live in their own files (see `xero_store`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRec {
    pub id: WorkspaceId,
    pub name: String,
    pub root: PathBuf,
    pub streams: Vec<StreamId>,
}

impl WorkspaceRec {
    /// Register `root`, taking the display name from its final path component.
    pub fn new(root: PathBuf) -> Self {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());
        WorkspaceRec { id: WorkspaceId::new(), name, root, streams: Vec::new() }
    }
}

/// The root persisted record (backs `~/.xero/workspaces.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "current_version")]
    pub version: u32,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRec>,
    #[serde(default)]
    pub active: Option<Active>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry { version: CURRENT_VERSION, workspaces: Vec::new(), active: None }
    }
}

/// Which workspace/stream was focused when the app last closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Active {
    pub workspace: WorkspaceId,
    pub stream: StreamId,
}

pub const CURRENT_VERSION: u32 = 1;

fn current_version() -> u32 {
    CURRENT_VERSION
}

impl Registry {
    pub fn workspace(&self, id: WorkspaceId) -> Option<&WorkspaceRec> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut WorkspaceRec> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }
}
