//! Device labels: sanitize what the phone sends; shorten for the Mac banner.

use std::time::{SystemTime, UNIX_EPOCH};

/// "iPhone · Home Screen" → "iPhone" for the terminal banner.
pub fn device_short_name(label: &str) -> &str {
    label.split(" · ").next().unwrap_or(label).trim()
}

pub(crate) fn clean_label(label: &str) -> String {
    let label: String = label.chars().filter(|c| !c.is_control()).take(40).collect();
    let label = label.trim();
    if label.is_empty() {
        "Phone".to_string()
    } else {
        label.to_string()
    }
}

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
