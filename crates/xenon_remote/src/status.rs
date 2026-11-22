use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use xenon_store::RemoteDevice;

use crate::frame::Rgb;
use crate::protocol::ThemeWire;

pub const KEEP_AWAKE_LABEL: &str = "Keep this computer awake while a phone is connected";
pub const KEEP_AWAKE_DETAIL: &str = "Ignores your sleep setting until the phone disconnects";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRow {
    pub id: String,
    pub label: String,
    pub connected: bool,
    pub last_seen_at: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteStatus {
    pub enabled: bool,
    #[serde(rename = "waiting")]
    old_windows_waiting: bool,
    pub problem: Option<String>,
    pub keep_awake: bool,
    pub hostname: String,
    pub base_url: Option<String>,
    pub reachable: bool,
    pub devices: Vec<DeviceRow>,
    pub connected: usize,
}

impl RemoteStatus {
    pub fn hold_awake(&self) -> bool {
        self.enabled && self.keep_awake && self.connected > 0
    }

    pub fn on(&self) -> bool {
        self.enabled || self.problem.is_some()
    }

    pub fn subtitle(&self, off_hint: &str) -> String {
        if let Some(problem) = &self.problem {
            return format!("On, but it didn’t start: {problem}");
        }
        match (&self.base_url, self.reachable) {
            (Some(url), true) => format!("On — reachable at {}", url.trim_start_matches("http://")),
            (Some(_), false) => {
                "On — Tailscale not detected, so your phone can’t reach this computer".into()
            }
            (None, _) => off_hint.to_string(),
        }
    }
}

pub fn device_rows(devices: &[RemoteDevice], connected: impl Fn(&str) -> bool) -> Vec<DeviceRow> {
    devices
        .iter()
        .map(|d| DeviceRow {
            connected: connected(&d.id),
            id: d.id.clone(),
            label: d.label.clone(),
            last_seen_at: d.last_seen_at,
            created_at: d.created_at,
        })
        .collect()
}

/// The phone remote's port: `XENON_REMOTE_PORT`, else `services.json`.
pub fn effective_port() -> u16 {
    let saved = xenon_store::load_service_settings()
        .unwrap_or_default()
        .remote_port;
    std::env::var("XENON_REMOTE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if saved == 0 {
            xenon_store::DEFAULT_REMOTE_PORT
        } else {
            saved
        })
}

pub fn theme_wire([bg, fg, cursor]: [[u8; 3]; 3]) -> ThemeWire {
    let hex = |c: [u8; 3]| Rgb(c[0], c[1], c[2]).hex();
    ThemeWire {
        bg: hex(bg),
        fg: hex(fg),
        cursor: hex(cursor),
    }
}

pub fn ago(unix: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(unix);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => plural(secs / 3600, "hour"),
        _ => plural(secs / 86_400, "day"),
    }
}

fn plural(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}
