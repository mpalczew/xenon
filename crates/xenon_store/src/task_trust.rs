//! Per-workspace approval for `runOptions.runOn: "folderOpen"` tasks. Lives in
//! Xenon's data dir, never in the repo. An approval names the hash of the
//! tasks it covers (`xenon_core::folder_open_hash`), so an edited tasks.json
//! asks again.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, read_json, write_atomic};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTrustRecord {
    pub hash: String,
    pub allowed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TaskTrustFile {
    #[serde(default)]
    workspaces: BTreeMap<String, TaskTrustRecord>,
}

/// What the user decided for this workspace's folderOpen tasks, if the
/// decision still covers `hash`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskTrust {
    Allowed,
    Denied,
    Unknown,
}

fn path() -> std::path::PathBuf {
    data_dir().join("task_trust.json")
}

fn load() -> TaskTrustFile {
    let path = path();
    if !path.exists() {
        return TaskTrustFile::default();
    }
    read_json(&path).unwrap_or_default()
}

pub fn task_trust(root: &Path, hash: &str) -> TaskTrust {
    match load().workspaces.get(&root.display().to_string()) {
        Some(rec) if rec.hash == hash && rec.allowed => TaskTrust::Allowed,
        Some(rec) if rec.hash == hash => TaskTrust::Denied,
        _ => TaskTrust::Unknown,
    }
}

pub fn set_task_trust(root: &Path, hash: &str, allowed: bool) -> Result<(), StoreError> {
    let mut file = load();
    file.workspaces.insert(
        root.display().to_string(),
        TaskTrustRecord {
            hash: hash.to_string(),
            allowed,
        },
    );
    write_atomic(&path(), &file)
}
