//! Paired phone-remote devices (`remote_devices.json`, mode 0600). Only a
//! SHA-256 of each device token is stored.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, load_or_default, write_atomic};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub id: String,
    pub label: String,
    pub token_sha256: String,
    pub created_at: u64,
    pub last_seen_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevices {
    #[serde(default)]
    pub devices: Vec<RemoteDevice>,
}

const FILE: &str = "remote_devices.json";
const SLOT_DIRS: &[&str] = &[".xenon-a", ".xenon-b"];

/// State every instance of this user shares: `XENON_SHARED_DIR`, else the
/// `.xenon-shared` sibling of an a/b slot dir, else the data dir.
pub(crate) fn shared_dir() -> PathBuf {
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

pub fn load_remote_devices() -> Result<RemoteDevices, StoreError> {
    load_or_default(&devices_path())
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
