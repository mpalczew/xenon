//! Persistence for xero: the workspace registry and per-stream session files,
//! stored as JSON under `data_dir()` (`~/.xero`). Writes are atomic (temp file
//! plus rename); a corrupt file is backed up and defaults are returned so a bad
//! file never blocks startup.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use xero_core::{Registry, Stream, StreamId, WorkspaceId};

/// Durable UI settings under `settings.json`. All fields default for forward-compat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub vim_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            show_line_numbers: true,
            vim_mode: false,
        }
    }
}

fn default_font_size() -> f32 {
    14.0
}

fn default_true() -> bool {
    true
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{path} was corrupt; backed up to {backup}")]
    Corrupt { path: PathBuf, backup: PathBuf },
}

/// Root directory for xero state. Overridable via `XERO_DATA_DIR` (tests).
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XERO_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".xero")
}

fn registry_path() -> PathBuf {
    data_dir().join("workspaces.json")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Load app settings, or defaults if missing/corrupt.
pub fn load_settings() -> Result<AppSettings, StoreError> {
    load_or_default(&settings_path())
}

pub fn save_settings(settings: &AppSettings) -> Result<(), StoreError> {
    write_atomic(&settings_path(), settings)
}

fn session_path(workspace: WorkspaceId, stream: StreamId) -> PathBuf {
    data_dir()
        .join("streams")
        .join(workspace.to_string())
        .join(format!("{stream}.json"))
}

/// Load the registry, or a default if none exists. A corrupt file is preserved
/// as `*.corrupt` and a default is returned alongside the error's backup path.
pub fn load_registry() -> Result<Registry, StoreError> {
    load_or_default(&registry_path())
}

pub fn save_registry(registry: &Registry) -> Result<(), StoreError> {
    write_atomic(&registry_path(), registry)
}

pub fn load_session(workspace: WorkspaceId, stream: StreamId) -> Result<Stream, StoreError> {
    let path = session_path(workspace, stream);
    read_json(&path)
}

pub fn save_session(workspace: WorkspaceId, stream: &Stream) -> Result<(), StoreError> {
    write_atomic(&session_path(workspace, stream.id), stream)
}

/// Remove a stream's session file. Missing file is not an error (already gone).
pub fn delete_session(workspace: WorkspaceId, stream: StreamId) -> Result<(), StoreError> {
    let path = session_path(workspace, stream);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn load_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T, StoreError> {
    if !path.exists() {
        return Ok(T::default());
    }
    match read_json(path) {
        Ok(value) => Ok(value),
        Err(StoreError::Corrupt { .. }) => Ok(T::default()),
        Err(other) => Err(other),
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, StoreError> {
    let contents = fs::read_to_string(path)?;
    serde_json::from_str(&contents).map_err(|_| {
        let backup = path.with_extension("corrupt");
        let _ = fs::rename(path, &backup);
        StoreError::Corrupt {
            path: path.to_path_buf(),
            backup,
        }
    })
}

fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).expect("serializable state");
    let temp = path.with_extension("tmp");
    fs::write(&temp, json)?;
    fs::rename(&temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests;
