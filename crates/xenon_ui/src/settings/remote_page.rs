//! Settings › Phone Remote: on/off, network, keep-awake opt-in, address override,
//! pairing, and paired devices (revoke one or all).

use std::rc::Rc;

use gpui::{App, SharedString};
use xenon_store::RemoteNetwork;

use super::row::{ActionKind, Control, Field, Group, RowAction, SettingRow, Tone};
use crate::app::remote::{self, DeviceRow, MobileRemoteInfo};

const NETWORKS: [RemoteNetwork; 2] = [RemoteNetwork::Tailscale, RemoteNetwork::TailscaleAndLan];

pub(super) fn groups(cx: &App) -> Vec<Group> {
    let info = remote::mobile_remote_info(cx);
    vec![
        Group::untitled(server_rows(&info)),
        Group {
            title: Some("Paired devices".into()),
            rows: device_rows(&info.devices),
        },
    ]
}

/// Sidebar badge: the remote is on (or queued behind the other slot).
pub(super) fn is_on(cx: &App) -> bool {
    let info = remote::mobile_remote_info(cx);
    info.enabled || info.waiting
}

fn server_rows(info: &MobileRemoteInfo) -> Vec<SettingRow> {
    let port = remote::effective_port(xenon_store::load_settings().unwrap_or_default().remote_port);
    let selected = NETWORKS
        .iter()
        .position(|n| *n == info.network)
        .unwrap_or(0);
    vec![
        SettingRow::new(
            "phone-remote",
            "Phone remote",
            Control::Switch {
                on: info.enabled || info.waiting,
                toggle: Rc::new(|_, cx| remote::settings_toggle_remote(cx)),
            },
        )
        .detail(remote_subtitle(info))
        .keywords("mobile iphone"),
        SettingRow::new(
            "phone-network",
            "Network",
            Control::Choice {
                options: vec!["Tailscale".into(), "Tailscale + LAN".into()],
                selected,
                pick: Rc::new(|index, _, cx| remote::settings_set_network(NETWORKS[index], cx)),
            },
        )
        .detail("LAN also opens the port on your Wi-Fi")
        .keywords("wifi lan tailscale"),
        SettingRow::new(
            "phone-keep-awake",
            remote::KEEP_AWAKE_LABEL,
            Control::Switch {
                on: info.keep_awake,
                toggle: Rc::new(|_, cx| remote::settings_toggle_keep_awake(cx)),
            },
        )
        .detail(remote::KEEP_AWAKE_DETAIL)
        .keywords("sleep"),
        SettingRow::new(
            "phone-hostname",
            "Address override",
            Control::Field {
                field: Field::RemoteHostname,
                value: info.hostname.clone(),
                shown: if info.hostname.is_empty() {
                    "Automatic (MagicDNS name or Tailscale IP)".into()
                } else {
                    info.hostname.clone().into()
                },
            },
        )
        .detail("Host in the pairing QR code")
        .keywords("hostname url magicdns"),
        SettingRow::new(
            "phone-port",
            "Port",
            Control::Value(port.to_string().into()),
        )
        .detail("Stays the same across restarts, so paired phones keep working"),
        SettingRow::new(
            "phone-connect",
            "Pair a phone",
            Control::Action(RowAction {
                label: xenon_design_system::shortcut_text("Connect Phone…  ⌘⇧M").into(),
                kind: ActionKind::Primary,
                run: Rc::new(|_, cx| remote::settings_connect_phone(cx)),
            }),
        )
        .detail("Shows a QR code to scan")
        .keywords("qr pair"),
    ]
}

fn remote_subtitle(info: &MobileRemoteInfo) -> String {
    if info.waiting {
        return "On — the other Xenon has the phone remote until it quits".into();
    }
    match (&info.base_url, info.reachable) {
        (Some(url), true) => format!("On — reachable at {}", url.trim_start_matches("http://")),
        (Some(_), false) => {
            "On — Tailscale not detected, so your phone can’t reach this Mac".into()
        }
        (None, _) => xenon_design_system::shortcut_text("Off — ⌘⇧M connects a phone"),
    }
}

fn device_rows(devices: &[DeviceRow]) -> Vec<SettingRow> {
    if devices.is_empty() {
        return vec![
            SettingRow::new("phone-no-devices", "No phones paired yet", Control::None)
                .detail("Connect Phone shows a QR code to scan")
                .keywords("device"),
        ];
    }
    let mut rows: Vec<SettingRow> = devices.iter().map(device_row).collect();
    rows.push(
        SettingRow::new(
            "phone-sign-out-all",
            "Sign out every device",
            Control::Action(RowAction {
                label: "Sign out all".into(),
                kind: ActionKind::Destructive,
                run: Rc::new(|_, cx| remote::settings_revoke_all(cx)),
            }),
        )
        .detail("Old QR codes stop working too")
        .keywords("device revoke"),
    );
    rows
}

fn device_row(device: &DeviceRow) -> SettingRow {
    let status = if device.connected {
        "● Connected now".to_string()
    } else {
        format!("Last seen {}", ago(device.last_seen_at))
    };
    let id = device.id.clone();
    SettingRow::new(
        SharedString::from(format!("phone-device-{}", device.id)),
        device.label.clone(),
        Control::Action(RowAction {
            label: "Revoke".into(),
            kind: ActionKind::Secondary,
            run: Rc::new(move |_, cx| remote::settings_revoke_device(id.clone(), cx)),
        }),
    )
    .detail(format!("{status} · paired {}", ago(device.created_at)))
    .tone(if device.connected {
        Tone::Good
    } else {
        Tone::Muted
    })
    .keywords("device iphone revoke")
}

/// "just now", "5 min ago", "3 hours ago", "2 days ago".
fn ago(unix: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn info(devices: usize) -> MobileRemoteInfo {
        MobileRemoteInfo {
            devices: (0..devices)
                .map(|i| DeviceRow {
                    id: i.to_string(),
                    label: "iPhone".into(),
                    connected: false,
                    last_seen_at: 0,
                    created_at: 0,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn devices_end_with_sign_out_all() {
        let rows = device_rows(&info(2).devices);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].id.as_ref(), "phone-sign-out-all");
    }

    #[test]
    fn no_devices_shows_an_empty_row() {
        let rows = device_rows(&[]);
        assert!(matches!(rows[0].control, Control::None));
    }
}
