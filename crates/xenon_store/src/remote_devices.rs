//! Paired phone-remote devices (`remote_devices.json`, mode 0600). Only a
//! SHA-256 of each device token is stored.
//!
//! Lives in `shared_dir()`, not the slot data dir: the a/b slot launcher runs
//! two instances that copy state at flip time, and a phone paired in one slot
//! must stay paired when the other slot takes over the remote port.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, load_or_default, write_atomic};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub id: String,
    /// e.g. "iPhone · Home Screen".
    pub label: String,
    pub token_sha256: String,
    /// Unix seconds.
    pub created_at: u64,
    pub last_seen_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevices {
    #[serde(default)]
    pub devices: Vec<RemoteDevice>,
}

const FILE: &str = "remote_devices.json";
/// Slot data dirs the launcher uses; their shared sibling is `.xenon-shared`.
const SLOT_DIRS: &[&str] = &[".xenon-a", ".xenon-b"];

/// State every instance of this user shares: `XENON_SHARED_DIR`, else the
/// `.xenon-shared` sibling of an a/b slot dir, else the data dir.
pub fn shared_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XENON_SHARED_DIR") {
        return PathBuf::from(dir);
    }
    let data = data_dir();
    let is_slot = data
        .file_name()
        .is_some_and(|name| SLOT_DIRS.iter().any(|slot| name == *slot));
    match (is_slot, data.parent()) {
        (true, Some(parent)) => parent.join(".xenon-shared"),
        _ => data,
    }
}

fn devices_path() -> PathBuf {
    shared_dir().join(FILE)
}

/// Paired devices, or none if the file is missing or corrupt. Falls back to
/// this slot's own file once, from before devices were shared.
pub fn load_remote_devices() -> Result<RemoteDevices, StoreError> {
    let shared = devices_path();
    let legacy = data_dir().join(FILE);
    if !shared.exists() && legacy.exists() {
        return load_or_default(&legacy);
    }
    load_or_default(&shared)
}

pub fn save_remote_devices(devices: &RemoteDevices) -> Result<(), StoreError> {
    let path = devices_path();
    write_atomic(&path, devices)?;
    restrict_to_owner(&path)
}

#[cfg(unix)]
fn restrict_to_owner(path: &std::path::Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &std::path::Path) -> Result<(), StoreError> {
    Ok(())
}
