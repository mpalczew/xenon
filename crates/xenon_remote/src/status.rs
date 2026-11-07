//! What Settings and the sidebar say about the remote: one status snapshot,
//! the footer states, and their wording. Frontends fill them in and draw.

use std::time::{SystemTime, UNIX_EPOCH};

use xenon_store::RemoteDevice;

use crate::frame::Rgb;
use crate::network::{base_url, normalize_hostname};
use crate::protocol::ThemeWire;

/// One paired device as Settings lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub label: String,
    pub connected: bool,
    pub last_seen_at: u64,
    pub created_at: u64,
}

/// Remote status snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteStatus {
    pub enabled: bool,
    /// On, but another Xenon holds the port for now.
    pub waiting: bool,
    /// Opt-in: hold the machine awake while a phone is connected.
    pub keep_awake: bool,
    /// User override for the phone URL host (empty = automatic).
    pub hostname: String,
    /// `http://host:port` the phone should open, when running.
    pub base_url: Option<String>,
    /// A phone can reach us from off this computer (over the tailnet).
    pub reachable: bool,
    pub devices: Vec<DeviceRow>,
    pub connected: usize,
}

impl RemoteStatus {
    /// Settings subtitle under the on/off switch. `off_hint` is the
    /// frontend's own "how to turn it on" line.
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

/// The phone URL for a running server.
pub fn phone_base_url(host_override: &str, magic_host: &str, port: u16) -> String {
    let host = normalize_hostname(host_override);
    base_url(if host.is_empty() { magic_host } else { &host }, port)
}

/// Device list rows; `connected` says whether a device has a live session.
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

/// The saved port, an `XENON_REMOTE_PORT` override, else `default`.
pub fn effective_port(saved: u16, default: u16) -> u16 {
    std::env::var("XENON_REMOTE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if saved == 0 { default } else { saved })
}

/// Terminal background, foreground and cursor colors for the phone page.
pub fn theme_wire([bg, fg, cursor]: [[u8; 3]; 3]) -> ThemeWire {
    let hex = |c: [u8; 3]| Rgb(c[0], c[1], c[2]).hex();
    ThemeWire {
        bg: hex(bg),
        fg: hex(fg),
        cursor: hex(cursor),
    }
}

/// "just now", "5 min ago", "3 hours ago", "2 days ago".
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
