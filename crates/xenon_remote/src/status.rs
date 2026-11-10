use std::time::{SystemTime, UNIX_EPOCH};

use xenon_store::RemoteDevice;

use crate::frame::Rgb;
use crate::protocol::ThemeWire;

pub const KEEP_AWAKE_LABEL: &str = "Keep this computer awake while a phone is connected";
pub const KEEP_AWAKE_DETAIL: &str = "Ignores your sleep setting until the phone disconnects";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub label: String,
    pub connected: bool,
    pub last_seen_at: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteStatus {
    pub enabled: bool,
    pub waiting: bool,
    pub keep_awake: bool,
    pub hostname: String,
    pub base_url: Option<String>,
    pub reachable: bool,
    pub devices: Vec<DeviceRow>,
    pub connected: usize,
}

impl RemoteStatus {
    /// Whether the computer should stay awake for a connected phone.
    pub fn hold_awake(&self) -> bool {
        self.enabled && self.keep_awake && self.connected > 0
    }

    pub fn subtitle(&self, off_hint: &str) -> String {
        if self.waiting {
            return "On — the other Xenon has the phone remote until it quits".into();
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

pub fn effective_port(saved: u16, default: u16) -> u16 {
    std::env::var("XENON_REMOTE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if saved == 0 { default } else { saved })
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
