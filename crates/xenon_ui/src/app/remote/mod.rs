//! Mobile PTY remote: host handlers + start/stop the local HTTP/WS server.

use super::*;

mod host;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::{Global, WeakEntity};
use xenon_remote::{HostRequest, RemoteServer};

/// Weak handle to the main shell so Settings (separate window) can toggle remote.
pub(crate) struct MainApp(pub WeakEntity<XenonApp>);
impl Global for MainApp {}

/// Live remote status for Settings.
/// Stored as a gpui Global so Settings paint never leases `XenonApp`
/// (opening Settings runs inside a XenonApp update → double_lease_panic).
#[derive(Clone, Debug, Default)]
pub(crate) struct MobileRemoteInfo {
    pub enabled: bool,
    pub port: u16,
    /// Shared password (same as phone token).
    pub token: String,
    /// Optional user hostname for the primary copy URL.
    pub hostname: String,
    /// Prefer custom host, then Tailscale / localhost; includes `?token=` for copy-paste.
    pub urls: Vec<String>,
}

impl Global for MobileRemoteInfo {}

pub(crate) fn mobile_remote_info(cx: &App) -> MobileRemoteInfo {
    cx.try_global::<MobileRemoteInfo>()
        .cloned()
        .unwrap_or_else(info_from_disk)
}

fn publish_remote_info(info: MobileRemoteInfo, cx: &mut App) {
    cx.set_global(info);
}

fn info_from_disk() -> MobileRemoteInfo {
    let s = xenon_store::load_settings().unwrap_or_default();
    let port = effective_port(s.remote_port);
    let token = s.remote_password;
    let hostname = normalize_hostname(&s.remote_hostname);
    MobileRemoteInfo {
        enabled: false,
        port,
        token: token.clone(),
        hostname: hostname.clone(),
        urls: if token.is_empty() {
            Vec::new()
        } else {
            remote_urls_for(port, &token, &hostname)
        },
    }
}

fn effective_port(saved: u16) -> u16 {
    std::env::var("XENON_REMOTE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if saved == 0 {
            xenon_store::DEFAULT_REMOTE_PORT
        } else {
            saved
        })
}

/// Ensure a password exists on disk; generate once if empty. Returns (port, password).
fn ensure_remote_credentials() -> (u16, String) {
    let s = xenon_store::update_settings(|settings| {
        settings.remote_port = effective_port(settings.remote_port);
        if settings.remote_password.is_empty() {
            settings.remote_password = xenon_remote::new_token();
        }
    })
    .unwrap_or_else(|e| {
        log::error!("save remote credentials failed: {e}");
        xenon_store::AppSettings::default()
    });
    let port = effective_port(s.remote_port);
    (port, s.remote_password)
}

/// Persist a user-chosen password. Restarts the remote server if it is running.
pub(crate) fn set_remote_password(password: String, window: &mut Window, cx: &mut App) {
    let password = password.trim().to_string();
    if password.is_empty() {
        return;
    }
    if let Err(e) = xenon_store::update_settings(|settings| {
        settings.remote_password = password.clone();
    }) {
        log::error!("save remote password failed: {e}");
        return;
    }
    if let Some(main) = cx.try_global::<MainApp>().map(|m| m.0.clone()) {
        let _ = main.update(cx, |app, cx| {
            let was_on = app.services.remote.is_some();
            if was_on {
                app.stop_mobile_remote(cx);
                app.start_mobile_remote(window, cx, false);
            } else {
                publish_remote_info(app.snapshot_remote_info(), cx);
                app.refresh_settings_window(cx);
            }
        });
    }
}

/// Persist hostname for copyable URLs (server bind unchanged — no restart).
pub(crate) fn set_remote_hostname(hostname: String, cx: &mut App) {
    let hostname = normalize_hostname(&hostname);
    if let Err(e) = xenon_store::update_settings(|settings| {
        settings.remote_hostname = hostname.clone();
    }) {
        log::error!("save remote hostname failed: {e}");
        return;
    }
    if let Some(main) = cx.try_global::<MainApp>().map(|m| m.0.clone()) {
        let _ = main.update(cx, |app, cx| {
            publish_remote_info(app.snapshot_remote_info(), cx);
            app.refresh_settings_window(cx);
        });
    } else {
        publish_remote_info(info_from_disk(), cx);
    }
}

/// Strip scheme/path noise; keep host only (optional port in host:port is allowed).
fn normalize_hostname(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let s = s.split('/').next().unwrap_or(s).trim();
    // Drop accidental trailing colon with no port.
    s.trim_end_matches(':').to_string()
}

/// Build `http://host:port/?token=…` candidates for the phone UI.
pub(crate) fn remote_urls_for(port: u16, token: &str, hostname: &str) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    let custom = normalize_hostname(hostname);
    if !custom.is_empty() {
        hosts.push(custom);
    }
    if let Some(ip) = tailscale_ipv4()
        && !hosts.iter().any(|h| h == &ip)
    {
        hosts.push(ip);
    }
    // Always include loopback for same-machine smoke tests.
    if !hosts.iter().any(|h| h == "127.0.0.1") {
        hosts.push("127.0.0.1".into());
    }
    let q = percent_encode_query(token);
    hosts
        .into_iter()
        .map(|h| {
            // If user typed host:port, don't append our port again.
            if h.contains(':') && !h.starts_with('[') {
                format!("http://{h}/?token={q}")
            } else {
                format!("http://{h}:{port}/?token={q}")
            }
        })
        .collect()
}

/// RFC 3986 unreserved + encode the rest (for password in query).
fn percent_encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn tailscale_ipv4() -> Option<String> {
    let out = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let ip = s.lines().next()?.trim();
    if ip.is_empty() || !ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some(ip.to_string())
}

impl XenonApp {
    /// Register so Settings can reach the main app entity.
    pub(crate) fn register_main_handle(cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        cx.set_global(MainApp(weak));
        publish_remote_info(info_from_disk(), cx);
    }

    fn snapshot_remote_info(&self) -> MobileRemoteInfo {
        let hostname = xenon_store::load_settings()
            .map(|s| normalize_hostname(&s.remote_hostname))
            .unwrap_or_default();
        match &self.services.remote {
            Some(s) => MobileRemoteInfo {
                enabled: true,
                port: s.port,
                token: s.token.clone(),
                hostname: hostname.clone(),
                urls: remote_urls_for(s.port, &s.token, &hostname),
            },
            None => info_from_disk(),
        }
    }

    /// Toggle mobile remote server (off by default).
    pub(crate) fn toggle_mobile_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mobile_remote_with_prompt(window, cx, true);
    }

    /// Toggle from Settings (details shown in the settings UI; skip info prompts).
    pub(crate) fn toggle_mobile_remote_quiet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_mobile_remote_with_prompt(window, cx, false);
    }

    fn toggle_mobile_remote_with_prompt(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        announce: bool,
    ) {
        if self.services.remote.is_some() {
            self.stop_mobile_remote(cx);
            if announce {
                let answer = window.prompt(
                    PromptLevel::Info,
                    "Mobile remote stopped",
                    None,
                    &["OK"],
                    cx,
                );
                cx.spawn(async move |_, _| {
                    let _ = answer.await;
                })
                .detach();
            }
        } else {
            self.start_mobile_remote(window, cx, announce);
        }
    }

    pub(crate) fn start_mobile_remote(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        announce: bool,
    ) {
        if self.services.remote.is_some() {
            return;
        }
        let (tx, rx) = async_channel::unbounded::<HostRequest>();
        let (port, token) = ensure_remote_credentials();
        match RemoteServer::start(port, token, tx) {
            Ok(server) => {
                let token = server.token.clone();
                let port = server.port;
                let hostname = xenon_store::load_settings()
                    .map(|s| normalize_hostname(&s.remote_hostname))
                    .unwrap_or_default();
                let url_hint = remote_urls_for(port, &token, &hostname)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| format!("http://127.0.0.1:{port}/?token={token}"));
                log::info!("Mobile remote ON — {url_hint}");
                if announce {
                    let detail = format!(
                        "URL (includes password):\n{url_hint}\n\nTerminals only · phone does not resize PTY."
                    );
                    let answer = window.prompt(
                        PromptLevel::Info,
                        "Mobile remote enabled",
                        Some(&detail),
                        &["OK"],
                        cx,
                    );
                    cx.spawn(async move |_, _| {
                        let _ = answer.await;
                    })
                    .detach();
                }
                self.services.remote = Some(server);
                self.services.remote_frame_seq = Arc::new(Mutex::new(HashMap::new()));
                self.spawn_remote_host(rx, cx);
            }
            Err(e) => {
                log::error!("mobile remote failed to start: {e:#}");
                let msg = format!("Mobile remote failed: {e}");
                let answer = window.prompt(PromptLevel::Critical, &msg, None, &["OK"], cx);
                cx.spawn(async move |_, _| {
                    let _ = answer.await;
                })
                .detach();
            }
        }
        publish_remote_info(self.snapshot_remote_info(), cx);
        self.refresh_settings_window(cx);
        cx.notify();
    }

    pub(crate) fn stop_mobile_remote(&mut self, cx: &mut Context<Self>) {
        if let Some(server) = self.services.remote.take() {
            server.stop();
            drop(server);
            log::info!("Mobile remote OFF");
        }
        self.services.remote_task = None;
        self.services.remote_frame_seq = Arc::new(Mutex::new(HashMap::new()));
        publish_remote_info(info_from_disk(), cx);
        self.refresh_settings_window(cx);
        cx.notify();
    }

    fn refresh_settings_window(&self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            let _ = handle.update(cx, |_, _, cx| cx.notify());
        }
    }

    fn spawn_remote_host(
        &mut self,
        rx: async_channel::Receiver<HostRequest>,
        cx: &mut Context<Self>,
    ) {
        self.services.remote_task = Some(cx.spawn(async move |view, cx| {
            while let Ok(req) = rx.recv().await {
                if view
                    .update(cx, |app, cx| app.handle_remote_request(req, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn handle_remote_request(&mut self, req: HostRequest, cx: &mut Context<Self>) {
        match req {
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(self.list_remote_workspaces());
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                let out = self.list_remote_terminals(&workspace_id, cx);
                let _ = reply.send(out);
            }
            HostRequest::CaptureFrame {
                workspace_id,
                tab_id,
                reply,
            } => {
                let out = self.capture_remote_frame(&workspace_id, tab_id, cx);
                let _ = reply.send(out);
            }
            HostRequest::Inject {
                workspace_id,
                tab_id,
                text,
                reply,
            } => {
                let out = self.inject_remote(&workspace_id, tab_id, &text, cx);
                let _ = reply.send(out);
            }
        }
    }
}
