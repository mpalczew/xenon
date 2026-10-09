//! Workspaces and the persisted registry that indexes them.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ids::WorkspaceId;

/// A registered project folder. Session details live in their own files
/// (see `xenon_store`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRec {
    pub id: WorkspaceId,
    pub name: String,
    pub root: PathBuf,
    /// SSH roots use a virtual namespace, never a local filesystem directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<xenon_ssh::SshWorkspace>,
    /// Unix seconds when this workspace was last activated (MRU for ⌘⇧O).
    /// Absent on pre-field registry JSON → treated as never opened for sort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened: Option<u64>,
}

impl WorkspaceRec {
    pub fn remote(ssh: xenon_ssh::SshWorkspace) -> Self {
        let id = WorkspaceId::new();
        let name = std::path::Path::new(&ssh.directory)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| ssh.host.clone());
        Self {
            id,
            name,
            root: PathBuf::from("/__xenon_ssh__").join(id.to_string()),
            ssh: Some(ssh),
            last_opened: Some(now_unix()),
        }
    }

    pub fn display_root(&self) -> String {
        self.ssh
            .as_ref()
            .map(|ssh| format!("{}:{}", ssh.host, ssh.directory))
            .unwrap_or_else(|| self.root.display().to_string())
    }

    /// Register `root`, taking the **default** display name from its final path
    /// component. Callers may rename `name` later without changing `root`.
    pub fn new(root: PathBuf) -> Self {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());
        WorkspaceRec {
            id: WorkspaceId::new(),
            name,
            root,
            ssh: None,
            last_opened: Some(now_unix()),
        }
    }

    /// Mark as most-recently opened (activate / reopen).
    pub fn touch_opened(&mut self) {
        self.last_opened = Some(now_unix());
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The root persisted record (backs `~/.xenon/workspaces.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "current_version")]
    pub version: u32,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRec>,
    #[serde(default)]
    pub closed_workspaces: Vec<WorkspaceRec>,
    #[serde(default)]
    pub active: Option<Active>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry {
            version: CURRENT_VERSION,
            workspaces: Vec::new(),
            closed_workspaces: Vec::new(),
            active: None,
        }
    }
}

/// Which workspace was focused when the app last closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Active {
    pub workspace: WorkspaceId,
}

pub const CURRENT_VERSION: u32 = 2;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_workspace_identity_survives_registry_round_trip() {
        let record: WorkspaceRec = serde_json::from_value(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001", "name": "project",
            "root": "/__xenon_ssh__/00000000-0000-0000-0000-000000000001",
            "ssh": { "host": "devbox", "directory": "/home/alex/project" }
        }))
        .unwrap();
        let restored: WorkspaceRec =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(restored, record);
        assert_eq!(restored.display_root(), "devbox:/home/alex/project");
    }
}
