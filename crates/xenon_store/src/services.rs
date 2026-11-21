use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, load_or_default, write_atomic};

pub const DEFAULT_REMOTE_PORT: u16 = 17890;
pub const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceSettings {
    pub remote_enabled: bool,
    pub remote_keep_awake_while_connected: bool,
    pub remote_port: u16,
    pub remote_hostname: String,
    pub terminal_scrollback_lines: usize,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            remote_enabled: false,
            remote_keep_awake_while_connected: false,
            remote_port: DEFAULT_REMOTE_PORT,
            remote_hostname: String::new(),
            terminal_scrollback_lines: DEFAULT_SCROLLBACK_LINES,
        }
    }
}

pub fn service_settings_path() -> PathBuf {
    data_dir().join("services.json")
}

pub fn load_service_settings() -> Result<ServiceSettings, StoreError> {
    load_or_default(&service_settings_path())
}

pub fn update_service_settings(
    update: impl FnOnce(&mut ServiceSettings),
) -> Result<ServiceSettings, StoreError> {
    let _guard = crate::SETTINGS_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut settings = load_service_settings()?;
    update(&mut settings);
    write_atomic(&service_settings_path(), &settings)?;
    Ok(settings)
}
