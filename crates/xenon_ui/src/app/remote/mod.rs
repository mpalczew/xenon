//! Phone remote: server lifecycle, request dispatch, and the status snapshot
//! Settings and the sidebar read. Sessions (attach/frames/input) live in
//! `session`; lists in `host`; pairing UI in `pairing_sheet`.

use super::*;

mod appearance;
mod feed;
mod host;
mod keep_awake;
pub(crate) mod pairing_sheet;
mod session;
mod settings_api;
mod status;
mod tasks;
#[cfg(feature = "visual-tests")]
mod visual;
mod wire;

use std::collections::BTreeMap;

use gpui::{Global, WeakEntity};
use xenon_remote::{
    ConnId, DeviceBook, HostRequest, Inhibitor, PairError, PairResponse, Reachability,
    RemoteServer, StartError,
};

use session::Conn;
pub(crate) use settings_api::*;
pub(crate) use xenon_remote::{DeviceRow, KEEP_AWAKE_DETAIL, KEEP_AWAKE_LABEL, normalize_hostname};

/// How often the remote re-checks dots, the pairing countdown, and the network.
const TICK: Duration = Duration::from_secs(1);
/// Network re-probe cadence, in ticks.
const NETWORK_EVERY: u32 = 15;
/// While the other a/b slot holds the port, try to take it over this often.
const PORT_RETRY: Duration = Duration::from_secs(5);

/// Weak handle to the main shell so Settings (separate window) can reach it.
pub(crate) struct MainApp(pub WeakEntity<XenonApp>);
impl Global for MainApp {}

/// Remote status snapshot. A gpui Global so Settings paint never leases
/// `XenonApp` (opening Settings runs inside a XenonApp update).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MobileRemoteInfo(pub xenon_remote::RemoteStatus);

impl std::ops::Deref for MobileRemoteInfo {
    type Target = xenon_remote::RemoteStatus;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Global for MobileRemoteInfo {}

pub(crate) fn mobile_remote_info(cx: &App) -> MobileRemoteInfo {
    cx.try_global::<MobileRemoteInfo>()
        .cloned()
        .unwrap_or_default()
}

/// Everything alive while the remote is on. Dropping it stops the server,
/// releases the keep-awake hold, and ends the background tasks.
pub(crate) struct RemoteRuntime {
    /// Held for its `Drop`, which stops listening.
    _server: RemoteServer,
    /// Port the phone URL uses (the server's; fixed in visual tests).
    port: u16,
    reach: Reachability,
    host_name: String,
    devices: DeviceBook,
    conns: BTreeMap<ConnId, Conn>,
    /// Held only while a phone is connected and the user opted in.
    keep_awake: Inhibitor,
    _requests: Task<()>,
    _tick: Task<()>,
    ticks: u32,
}

impl XenonApp {
    /// Register so Settings can reach the main app; start if left on last time.
    pub(crate) fn register_main_handle(&mut self, window: &Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        cx.set_global(MainApp(weak));
        self.services.main_window = Some(window.window_handle());
        let settings = xenon_store::load_settings().unwrap_or_default();
        if settings.remote_enabled && !self.skip_persist {
            self.start_mobile_remote(cx);
        }
        self.publish_remote_info(cx);
    }

    pub(crate) fn remote_running(&self) -> bool {
        self.services.remote.is_some()
    }

    /// On but queued behind another Xenon that holds the port.
    pub(crate) fn remote_waiting(&self) -> bool {
        self.services.remote_port_wait.is_some()
    }

    /// Palette / menu toggle. Remembers the choice for the next launch.
    pub(crate) fn toggle_mobile_remote(&mut self, cx: &mut Context<Self>) {
        if self.remote_running() || self.remote_waiting() {
            self.stop_mobile_remote(cx);
        } else {
            self.start_mobile_remote(cx);
        }
        let on = self.remote_running() || self.remote_waiting();
        persist_enabled(on);
        let toast = match (self.remote_running(), on) {
            (true, _) => xenon_design_system::Toast::info("📱", "Phone remote on"),
            (false, true) => xenon_design_system::Toast::info(
                "📱",
                "Phone remote on — the other Xenon has it until it quits",
            ),
            (false, false) => xenon_design_system::Toast::info("📴", "Phone remote off"),
        };
        self.show_toast(toast, cx);
    }

    /// Another Xenon (the other a/b slot) owns the port: take over when it
    /// lets go (quits or turns its remote off). Silent: this is normal.
    fn wait_for_remote_port(&mut self, cx: &mut Context<Self>) {
        if self.remote_waiting() {
            return;
        }
        if !self.services.remote_port_busy_logged {
            log::info!("phone remote: port busy, waiting for the other Xenon");
            self.services.remote_port_busy_logged = true;
        }
        self.services.remote_port_wait = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PORT_RETRY).await;
            this.update(cx, |app, cx| {
                app.services.remote_port_wait = None;
                app.start_mobile_remote(cx);
            })
            .ok();
        }));
    }

    pub(crate) fn start_mobile_remote(&mut self, cx: &mut Context<Self>) {
        if self.remote_running() {
            return;
        }
        let settings = xenon_store::load_settings().unwrap_or_default();
        let reach = Reachability::probe_local();
        let (tx, rx) = async_channel::unbounded::<HostRequest>();
        let port = effective_port(settings.remote_port);
        let server = match RemoteServer::start(&reach.bind_ips(), port, tx) {
            Ok(server) => server,
            Err(StartError::PortBusy) => {
                self.wait_for_remote_port(cx);
                self.publish_remote_info(cx);
                return;
            }
            Err(e) => {
                log::error!("phone remote failed to start: {e:#}");
                self.show_toast(toasts::failed("Phone remote didn’t start", e), cx);
                return;
            }
        };
        server.set_uploads_dir(xenon_store::data_dir().join("remote").join("uploads"));
        log::info!("phone remote on: {:?}", server.binds);
        self.services.remote_port_busy_logged = false;
        self.services.remote = Some(RemoteRuntime {
            port: server.port,
            _server: server,
            reach,
            host_name: String::new(),
            devices: DeviceBook::load(),
            conns: BTreeMap::new(),
            keep_awake: Inhibitor::default(),
            _requests: self.spawn_remote_requests(rx, cx),
            _tick: self.spawn_remote_tick(cx),
            ticks: 0,
        });
        self.refresh_remote_names(cx);
        self.publish_remote_info(cx);
    }

    pub(crate) fn stop_mobile_remote(&mut self, cx: &mut Context<Self>) {
        if self.services.remote_port_wait.take().is_some() {
            self.publish_remote_info(cx);
        }
        let Some(runtime) = self.services.remote.take() else {
            return;
        };
        for conn in runtime.conns.values() {
            self.release_phone_fit(conn, cx);
        }
        drop(runtime);
        self.pairing_sheet = None;
        log::info!("phone remote off");
        self.publish_remote_info(cx);
    }

    /// Restart with fresh settings (address / port changed).
    pub(crate) fn restart_mobile_remote(&mut self, cx: &mut Context<Self>) {
        if self.remote_running() {
            self.stop_mobile_remote(cx);
            self.start_mobile_remote(cx);
        }
    }

    /// Recompute the Settings/sidebar snapshot and repaint both windows.
    pub(crate) fn publish_remote_info(&mut self, cx: &mut Context<Self>) {
        let info = self.remote_info_snapshot();
        self.sync_keep_awake(&info);
        if cx.try_global::<MobileRemoteInfo>() != Some(&info) {
            cx.set_global(info);
            if let Some(handle) = self.settings_window {
                let _ = handle.update(cx, |_, _, cx| cx.notify());
            }
        }
        cx.notify();
    }

    fn remote_info_snapshot(&self) -> MobileRemoteInfo {
        let settings = xenon_store::load_settings().unwrap_or_default();
        let hostname = normalize_hostname(&settings.remote_hostname);
        let mut info = xenon_remote::RemoteStatus {
            enabled: self.remote_running(),
            waiting: self.remote_waiting(),
            keep_awake: settings.remote_keep_awake_while_connected,
            hostname: hostname.clone(),
            ..Default::default()
        };
        let devices = match &self.services.remote {
            Some(runtime) => {
                let host = runtime.reach.phone_host(&hostname);
                info.base_url = Some(xenon_remote::base_url(&host, runtime.port));
                info.reachable = runtime.reach.reachable();
                info.connected = runtime.conns.len();
                runtime.devices.devices().to_vec()
            }
            None => {
                xenon_store::load_remote_devices()
                    .unwrap_or_default()
                    .devices
            }
        };
        info.devices = xenon_remote::device_rows(&devices, |id| {
            self.services
                .remote
                .as_ref()
                .is_some_and(|r| r.conns.values().any(|c| c.device_id == id))
        });
        MobileRemoteInfo(info)
    }

    fn spawn_remote_requests(
        &self,
        rx: async_channel::Receiver<HostRequest>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let main = self.services.main_window;
        cx.spawn(async move |this, cx| {
            while let Ok(req) = rx.recv().await {
                let Some(main) = main else { break };
                let handled = main.update(cx, |_, window, cx| {
                    this.update(cx, |app, cx| app.handle_remote_request(req, window, cx))
                });
                if !matches!(handled, Ok(Ok(()))) {
                    break;
                }
            }
        })
    }

    fn spawn_remote_tick(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |app, cx| app.remote_tick(cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn remote_tick(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self.services.remote.as_mut() else {
            return;
        };
        runtime.ticks = runtime.ticks.wrapping_add(1);
        let probe_network = runtime.ticks % NETWORK_EVERY == 0;
        self.broadcast_remote_dots(cx);
        if self.pairing_sheet.is_some() {
            cx.notify();
        }
        if probe_network {
            self.reprobe_remote_network(cx);
        }
    }

    /// Tailscale came up / changed address: rebind so the phone can connect.
    fn reprobe_remote_network(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self.services.remote.as_ref() else {
            return;
        };
        let fresh = Reachability::probe_local();
        if fresh.tailscale_ip == runtime.reach.tailscale_ip {
            if runtime.reach.magic_dns.is_none() && fresh.tailscale_ip.is_some() {
                self.refresh_remote_names(cx);
            }
            return;
        }
        log::info!("phone remote: network changed, rebinding");
        self.restart_mobile_remote(cx);
    }

    /// MagicDNS name and computer name come from subprocesses: off-thread.
    fn refresh_remote_names(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let (dns, name) = cx
                .background_executor()
                .spawn(async {
                    (
                        xenon_remote::magic_dns_name(),
                        xenon_remote::computer_name(),
                    )
                })
                .await;
            this.update(cx, |app, cx| {
                if let Some(runtime) = app.services.remote.as_mut() {
                    runtime.reach.magic_dns = dns;
                    runtime.host_name = name;
                }
                app.publish_remote_info(cx);
            })
            .ok();
        })
        .detach();
    }

    fn handle_remote_request(
        &mut self,
        req: HostRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match req {
            HostRequest::Authenticate { token, reply } => {
                let id = self
                    .services
                    .remote
                    .as_mut()
                    .and_then(|r| r.devices.authenticate_touch(&token));
                let _ = reply.send(id);
            }
            HostRequest::Pair { code, label, reply } => {
                let _ = reply.send(self.pair_remote(&code, &label, cx));
            }
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(self.list_remote_workspaces());
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.list_remote_terminals(&workspace_id, cx));
            }
            HostRequest::ListTasks {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.list_remote_tasks(&workspace_id, cx));
            }
            HostRequest::CreateTerminal {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.open_remote_terminal(&workspace_id, cx));
            }
            HostRequest::CloseTerminal {
                workspace_id,
                tab_id,
                reply,
            } => {
                let _ = reply.send(self.close_remote_terminal(&workspace_id, tab_id, window, cx));
            }
            HostRequest::Connected {
                conn,
                device_id,
                out,
                input,
            } => self.remote_connected(conn, device_id, (out, input), cx),
            HostRequest::InputSent { conn } => self.remote_input_sent(conn, cx),
            HostRequest::Message { conn, msg } => self.remote_message(conn, msg, window, cx),
            HostRequest::Disconnected { conn } => self.remote_disconnected(conn, cx),
        }
    }

    fn pair_remote(
        &mut self,
        code: &str,
        label: &str,
        cx: &mut Context<Self>,
    ) -> Result<PairResponse, PairError> {
        let runtime = self.services.remote.as_mut().ok_or(PairError::Rejected)?;
        let host_name = runtime.host_name.clone();
        let reply = runtime.devices.pair_response(code, label, &host_name)?;
        let toast = xenon_design_system::Toast::success("📱", "Phone paired");
        self.show_toast(toast, cx);
        self.publish_remote_info(cx);
        Ok(reply)
    }
}

fn persist_enabled(on: bool) {
    if let Err(e) = xenon_store::update_settings(|s| s.remote_enabled = on) {
        log::warn!("save remote_enabled: {e}");
    }
}

pub(crate) fn effective_port(saved: u16) -> u16 {
    xenon_remote::effective_port(saved, xenon_store::DEFAULT_REMOTE_PORT)
}
