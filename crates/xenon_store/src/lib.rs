//! Persistence for Xenon: the workspace registry and per-workspace session files,
//! stored as JSON under `data_dir()` (`~/.xenon`). Writes are atomic (temp file
//! plus rename); a corrupt file is backed up and defaults are returned so a bad
//! file never blocks startup.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde::de::DeserializeOwned;
use xenon_core::{Registry, SessionState, WorkspaceId};

mod cli;
mod ipc;
mod prose;
mod remote_devices;
mod settings;
mod skill;
mod task_trust;
mod terminal;
pub use cli::{CliCommand, parse_cli};
pub use ipc::{
    IpcRequest, IpcResponse, OpenFileSpec, OpenPane, bind_server, parse_cli_paths, send_request,
    serve_forever, socket_path, try_handoff, try_handoff_request,
};
pub use prose::{
    MarkdownMode, ProseFilePrefs, ProseFont, ProseMarkers, ProseSettings, prose_file_prefs,
    set_prose_file_prefs,
};
pub use remote_devices::{
    RemoteDevice, RemoteDevices, load_remote_devices, save_remote_devices, shared_dir,
};
pub use settings::{
    AppSettings, DEFAULT_DARK_THEME, DEFAULT_FONT_FAMILY, DEFAULT_LIGHT_THEME, DEFAULT_REMOTE_PORT,
    DEFAULT_UI_FONT_FAMILY, LspCommand, LspSettings, ThemeMode, WindowGeometry, WindowState,
};
pub use skill::{
    SkillStatus, decline_skill, install_skill, refresh_on_launch, remove_skill,
    should_prompt_skill, skill_home, skill_status,
};
pub use task_trust::{TaskTrust, set_task_trust, task_trust};
pub use terminal::TerminalAutoClose;

/// Serializes tests that mutate process-global data-dir env vars.
#[cfg(test)]
pub(crate) static DATA_DIR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{path} was corrupt; backed up to {backup}")]
    Corrupt { path: PathBuf, backup: PathBuf },
}

/// Root directory for app state. Overridable via `XENON_DATA_DIR` (tests /
/// slot launcher). Default `~/.xenon`.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XENON_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".xenon")
}

fn registry_path() -> PathBuf {
    data_dir().join("workspaces.json")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Load app settings, or defaults if missing/corrupt.
pub fn load_settings() -> Result<AppSettings, StoreError> {
    let path = settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let contents = fs::read_to_string(&path)?;
    let value: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(value) => value,
        Err(_) => {
            let backup = path.with_extension("corrupt");
            let _ = fs::rename(&path, &backup);
            return Ok(AppSettings::default());
        }
    };
    match serde_json::from_value(value) {
        Ok(settings) => Ok(settings),
        Err(_) => {
            let backup = path.with_extension("corrupt");
            let _ = fs::rename(&path, &backup);
            Ok(AppSettings::default())
        }
    }
}

#[cfg(test)]
pub(crate) fn save_settings(settings: &AppSettings) -> Result<(), StoreError> {
    let _guard = SETTINGS_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    save_settings_unlocked(settings)
}

/// Atomically load, mutate, and save settings while preserving fields the
/// caller does not own.
pub fn update_settings(update: impl FnOnce(&mut AppSettings)) -> Result<AppSettings, StoreError> {
    let _guard = SETTINGS_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut settings = load_settings()?;
    update(&mut settings);
    save_settings_unlocked(&settings)?;
    Ok(settings)
}

fn save_settings_unlocked(settings: &AppSettings) -> Result<(), StoreError> {
    write_atomic(&settings_path(), settings)
}

fn session_path(workspace: WorkspaceId) -> PathBuf {
    data_dir()
        .join("sessions")
        .join(format!("{workspace}.json"))
}

/// Load the registry, or a default if none exists. A corrupt file is preserved
/// as `*.corrupt` and a default is returned alongside the error's backup path.
pub fn load_registry() -> Result<Registry, StoreError> {
    load_or_default(&registry_path())
}

pub fn save_registry(registry: &Registry) -> Result<(), StoreError> {
    write_atomic(&registry_path(), registry)
}

/// Load a workspace session from `sessions/<id>.json`.
pub fn load_session(workspace: WorkspaceId) -> Result<SessionState, StoreError> {
    let path = session_path(workspace);
    if path.exists() {
        return read_json(&path);
    }
    Err(StoreError::Io(io::Error::new(
        io::ErrorKind::NotFound,
        "session not found",
    )))
}

pub fn save_session(workspace: WorkspaceId, session: &SessionState) -> Result<(), StoreError> {
    write_atomic(&session_path(workspace), session)
}

/// Remove a workspace session file. Missing file is not an error (already gone).
pub fn delete_session(workspace: WorkspaceId) -> Result<(), StoreError> {
    let path = session_path(workspace);
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
