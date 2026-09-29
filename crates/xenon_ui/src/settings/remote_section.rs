//! Settings → Phone Remote: on/off, network, keep-awake, address override,
//! Connect Phone, and paired devices (revoke one or all).
//!
//! Keyboard: focus indices continue the Settings list (see `SettingsView::toggle_focus`).

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{ActionButton, TextInputView, TypeRole, Typography};
use xenon_store::RemoteNetwork;

use super::SettingsView;
use super::sections::{ToggleRow, group_card, row_divider, settings_toggle};
use crate::app::remote::{self, DeviceRow, MobileRemoteInfo};

pub(super) const FOCUS_REMOTE: usize = 2;
const FOCUS_NETWORK: usize = 4;
const FOCUS_KEEP_AWAKE: usize = 5;
pub(super) const FOCUS_CONNECT: usize = 6;
pub(super) const FOCUS_FIRST_DEVICE: usize = 7;

/// Highest focus index: the last device row, then "Sign out all".
pub(super) fn last_focus(info: &MobileRemoteInfo) -> usize {
    if info.devices.is_empty() {
        FOCUS_CONNECT
    } else {
        FOCUS_FIRST_DEVICE + info.devices.len()
    }
}

/// Enter / Space on a remote row.
pub(super) fn activate(focus: usize, info: &MobileRemoteInfo, cx: &mut App) {
    match focus {
        FOCUS_REMOTE => remote::settings_toggle_remote(cx),
        FOCUS_NETWORK => remote::settings_set_network(next_network(info.network), cx),
        FOCUS_KEEP_AWAKE => remote::settings_toggle_keep_awake(cx),
        FOCUS_CONNECT => remote::settings_connect_phone(cx),
        i if i >= FOCUS_FIRST_DEVICE => match info.devices.get(i - FOCUS_FIRST_DEVICE) {
            Some(device) => remote::settings_revoke_device(device.id.clone(), cx),
            None => remote::settings_revoke_all(cx),
        },
        _ => {}
    }
}

fn next_network(network: RemoteNetwork) -> RemoteNetwork {
    match network {
        RemoteNetwork::Tailscale => RemoteNetwork::TailscaleAndLan,
        RemoteNetwork::TailscaleAndLan => RemoteNetwork::Tailscale,
    }
}

pub(super) fn remote_section(
    info: &MobileRemoteInfo,
    focus: usize,
    hostname_edit: Option<gpui::Entity<TextInputView>>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let body = div()
        .flex()
        .flex_col()
        .child(settings_toggle(
            ToggleRow {
                id: "phone-remote-toggle",
                title: "Phone remote",
                subtitle: remote_subtitle(info).into(),
                checked: info.enabled || info.waiting,
                focused: focus == FOCUS_REMOTE,
            },
            cx,
            remote::settings_toggle_remote,
        ))
        .child(row_divider(cx))
        .child(network_row(info.network, focus == FOCUS_NETWORK, cx))
        .child(row_divider(cx))
        .child(settings_toggle(
            ToggleRow {
                id: "phone-keep-awake-toggle",
                title: "Keep Mac awake",
                subtitle: "Stops idle sleep while on. Closing the lid still sleeps.".into(),
                checked: info.keep_awake,
                focused: focus == FOCUS_KEEP_AWAKE,
            },
            cx,
            remote::settings_toggle_keep_awake,
        ))
        .child(row_divider(cx))
        .child(hostname_row(&info.hostname, hostname_edit, cx))
        .child(row_divider(cx))
        .child(connect_row(focus == FOCUS_CONNECT, cx));
    div()
        .flex()
        .flex_col()
        .child(group_card("Phone Remote", body, cx))
        .child(devices_card(&info.devices, focus, cx))
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
        (None, _) => "Off — ⌘⇧M connects a phone".into(),
    }
}

fn focus_bg(focused: bool, cx: &App) -> gpui::Hsla {
    if focused {
        cx.theme().colors().element_hover
    } else {
        gpui::transparent_black()
    }
}

fn network_row(
    network: RemoteNetwork,
    focused: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let option = |id: &'static str,
                  label: &'static str,
                  value: RemoteNetwork,
                  cx: &mut Context<SettingsView>| {
        let variant = if network == value {
            ActionButton::primary(label)
        } else {
            ActionButton::quiet(label)
        };
        xenon_design_system::action_button(id, variant, cx, move |_, window, cx| {
            cx.stop_propagation();
            remote::settings_set_network(value, cx);
            window.refresh();
        })
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .bg(focus_bg(focused, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().type_role(TypeRole::Body, cx).child("Network"))
                .child(
                    div()
                        .type_role(TypeRole::ControlLabel, cx)
                        .text_color(cx.theme().colors().text_muted)
                        .child("LAN also opens the port on your Wi-Fi"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .child(option(
                    "phone-net-tailscale",
                    "Tailscale",
                    RemoteNetwork::Tailscale,
                    cx,
                ))
                .child(option(
                    "phone-net-lan",
                    "Tailscale + LAN",
                    RemoteNetwork::TailscaleAndLan,
                    cx,
                )),
        )
}

fn hostname_row(
    current: &str,
    edit: Option<gpui::Entity<TextInputView>>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let label = div()
        .type_role(TypeRole::ControlLabel, cx)
        .text_color(colors.text_muted);
    let row = div().flex().flex_col().gap_1().px_3().py_2();
    if let Some(input) = edit {
        return row
            .child(label.child("Address override — ↵ save · Esc cancel"))
            .child(
                div()
                    .id("remote-hostname-edit")
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(colors.border_focused)
                    .bg(colors.elevated_surface_background)
                    .type_role(TypeRole::Code, cx)
                    .child(div().flex_1().min_w_0().child(input)),
            )
            .into_any_element();
    }
    let empty = current.is_empty();
    let current_owned = current.to_string();
    row.child(label.child("Address override — click to edit"))
        .child(
            div()
                .id("remote-hostname")
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.begin_hostname_edit(current_owned.clone(), window, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .type_role(TypeRole::Code, cx)
                        .text_color(if empty {
                            colors.text_muted
                        } else {
                            colors.text
                        })
                        .child(if empty {
                            "Automatic (MagicDNS name or Tailscale IP)".to_string()
                        } else {
                            current.to_string()
                        }),
                )
                .child(
                    div()
                        .type_role(TypeRole::ControlLabel, cx)
                        .text_color(colors.text_muted)
                        .child("Edit"),
                ),
        )
        .into_any_element()
}

fn connect_row(focused: bool, cx: &mut Context<SettingsView>) -> impl IntoElement {
    div()
        .flex()
        .justify_end()
        .px_3()
        .py_2()
        .bg(focus_bg(focused, cx))
        .child(xenon_design_system::action_button(
            "phone-connect",
            ActionButton::primary("Connect Phone…  ⌘⇧M"),
            cx,
            |_, window, cx| {
                cx.stop_propagation();
                remote::settings_connect_phone(cx);
                window.refresh();
            },
        ))
}

fn devices_card(
    devices: &[DeviceRow],
    focus: usize,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let mut body = div().flex().flex_col();
    if devices.is_empty() {
        body = body.child(
            div()
                .px_3()
                .py_2()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child("No phones paired yet. Connect Phone shows a QR code to scan."),
        );
        return group_card("Paired devices", body, cx);
    }
    for (i, device) in devices.iter().enumerate() {
        body = body
            .child(device_row(device, focus == FOCUS_FIRST_DEVICE + i, cx))
            .child(row_divider(cx));
    }
    let sign_out_focused = focus == FOCUS_FIRST_DEVICE + devices.len();
    body = body.child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_2()
            .bg(focus_bg(sign_out_focused, cx))
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child("Signs out every device and makes old QR codes stop working."),
            )
            .child(xenon_design_system::action_button(
                "phone-sign-out-all",
                ActionButton::destructive("Sign out all"),
                cx,
                |_, window, cx| {
                    cx.stop_propagation();
                    remote::settings_revoke_all(cx);
                    window.refresh();
                },
            )),
    );
    group_card("Paired devices", body, cx)
}

fn device_row(
    device: &DeviceRow,
    focused: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let status: SharedString = if device.connected {
        "● Connected now".into()
    } else {
        format!("Last seen {}", ago(device.last_seen_at)).into()
    };
    let id = device.id.clone();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .py_2()
        .bg(focus_bg(focused, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .type_role(TypeRole::Body, cx)
                        .child(device.label.clone()),
                )
                .child(
                    div()
                        .type_role(TypeRole::ControlLabel, cx)
                        .text_color(if device.connected {
                            colors.version_control_added
                        } else {
                            colors.text_muted
                        })
                        .child(format!("{status} · paired {}", ago(device.created_at))),
                ),
        )
        .child(xenon_design_system::action_button(
            SharedString::from(format!("phone-revoke-{}", device.id)),
            ActionButton::secondary("Revoke"),
            cx,
            move |_, window, cx| {
                cx.stop_propagation();
                remote::settings_revoke_device(id.clone(), cx);
                window.refresh();
            },
        ))
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
    fn focus_range_covers_devices_and_sign_out_all() {
        assert_eq!(last_focus(&info(0)), FOCUS_CONNECT);
        assert_eq!(last_focus(&info(2)), FOCUS_FIRST_DEVICE + 2);
    }

    #[test]
    fn network_toggles() {
        assert_eq!(
            next_network(RemoteNetwork::Tailscale),
            RemoteNetwork::TailscaleAndLan
        );
        assert_eq!(
            next_network(RemoteNetwork::TailscaleAndLan),
            RemoteNetwork::Tailscale
        );
    }
}
