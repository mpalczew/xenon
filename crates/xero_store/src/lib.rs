//! Persistence for xero: the workspace registry and per-stream session files,
//! stored as JSON under `data_dir()` (`~/.xero`). Writes are atomic (temp file
//! plus rename); a corrupt file is backed up and defaults are returned so a bad
//! file never blocks startup.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use xero_core::{Registry, Stream, StreamId, WorkspaceId};

mod ipc;
mod settings;
mod terminal;
pub use ipc::{
    IpcRequest, IpcResponse, bind_server, parse_cli_paths, send_request, serve_forever,
    socket_path, try_handoff,
};
pub use settings::{
    AppSettings, DEFAULT_DARK_THEME, DEFAULT_FONT_FAMILY, DEFAULT_LIGHT_THEME,
    DEFAULT_UI_FONT_FAMILY, ThemeMode,
};
pub use terminal::TerminalAutoClose;

/// Serializes tests that mutate process-global data-dir env vars.
#[cfg(test)]
pub(crate) static DATA_DIR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{path} was corrupt; backed up to {backup}")]
    Corrupt { path: PathBuf, backup: PathBuf },
}

/// Root directory for app state. Overridable via `XENON_DATA_DIR` or legacy
/// `XERO_DATA_DIR` (tests / slot launcher). Default `~/.xenon`; one-time
/// migrate from `~/.xero` when present.
pub fn data_dir() -> PathBuf {
    if let Some(dir) =
        std::env::var_os("XENON_DATA_DIR").or_else(|| std::env::var_os("XERO_DATA_DIR"))
    {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let modern = home.join(".xenon");
    let legacy = home.join(".xero");
    if !modern.exists() && legacy.exists() {
        match fs::rename(&legacy, &modern) {
            Ok(()) => log::info!(
                "migrated data dir {} -> {}",
                legacy.display(),
                modern.display()
            ),
            Err(error) => log::warn!(
                "could not migrate {} to {}: {error}; using legacy path",
                legacy.display(),
                modern.display()
            ),
        }
    }
    if modern.exists() || !legacy.exists() {
        modern
    } else {
        legacy
    }
}

fn registry_path() -> PathBuf {
    data_dir().join("workspaces.json")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Load app settings, or defaults if missing/corrupt.
/// Migrates legacy `font_size` into editor + terminal sizes when needed.
pub fn load_settings() -> Result<AppSettings, StoreError> {
    let path = settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let contents = fs::read_to_string(&path)?;
    let mut value: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(value) => value,
        Err(_) => {
            let backup = path.with_extension("corrupt");
            let _ = fs::rename(&path, &backup);
            return Ok(AppSettings::default());
        }
    };
    migrate_settings_value(&mut value);
    match serde_json::from_value(value) {
        Ok(settings) => Ok(settings),
        Err(_) => {
            let backup = path.with_extension("corrupt");
            let _ = fs::rename(&path, &backup);
            Ok(AppSettings::default())
        }
    }
}

/// Copy legacy shared `font_size` into the split editor/terminal fields.
fn migrate_settings_value(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(legacy) = obj.get("font_size").cloned() {
        obj.entry("editor_font_size").or_insert(legacy.clone());
        obj.entry("terminal_font_size").or_insert(legacy);
    }
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
