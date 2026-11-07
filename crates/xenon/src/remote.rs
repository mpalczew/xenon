//! The phone remote in the GTK app: server lifecycle and the status snapshot.
//! Requests are in `remote_host.rs`, sessions in `remote_session.rs`, terminal
//! mirroring in `remote_mirror.rs`, phone fit in `remote_fit.rs`.
//!
//! One server per machine: the port is the saved `remote_port`
//! (default 17890), so paired phones need no re-pairing. When the other Xenon
//! holds it, this one stays quiet ("served by the other Xenon") and tries
//! again every few seconds, never touching the other server.
//!
//! Idle cost: nothing runs while the remote is off. While on, there is one
//! network re-probe every 30 s and, when waiting for the port, one retry every
//! 5 s. A connected phone adds no timers; a terminal is mirrored only while a
//! phone is attached to it.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};
use xenon_core::{TabId, WorkspaceId};
use xenon_remote::{
    DeviceBook, HostRequest, Inhibitor, Reachability, RemoteServer, RemoteStatus, StartError,
};

use crate::app::App;
use crate::remote_fit::TabFit;
use crate::remote_session::Conn;

const PORT_RETRY: Duration = Duration::from_secs(5);
const NETWORK_EVERY: Duration = Duration::from_secs(30);

/// Everything alive while the server is up.
pub(crate) struct Runtime {
    /// Held for its `Drop`, which stops listening.
    _server: RemoteServer,
    pub port: u16,
    pub reach: Reachability,
    pub host_name: String,
    pub devices: DeviceBook,
    pub conns: BTreeMap<xenon_remote::ConnId, Conn>,
    keep_awake: Inhibitor,
    net_timer: Option<glib::SourceId>,
}

pub(crate) struct Remote {
    pub rt: RefCell<Option<Runtime>>,
    wait: RefCell<Option<glib::SourceId>>,
    wait_logged: Cell<bool>,
    pub fits: RefCell<HashMap<(WorkspaceId, TabId), TabFit>>,
    /// Told after every status change (the Settings page redraws from it).
    pub on_change: RefCell<Option<Rc<dyn Fn()>>>,
    pub connect_dialog: RefCell<Option<adw::Dialog>>,
}

impl Remote {
    pub(crate) fn new() -> Self {
        Self {
            rt: RefCell::new(None),
            wait: RefCell::new(None),
            wait_logged: Cell::new(false),
            fits: RefCell::default(),
            on_change: RefCell::new(None),
            connect_dialog: RefCell::new(None),
        }
    }

    pub(crate) fn running(&self) -> bool {
        self.rt.borrow().is_some()
    }

    pub(crate) fn waiting(&self) -> bool {
        self.wait.borrow().is_some()
    }
}

pub(crate) fn persist_enabled(on: bool) {
    if let Err(error) = xenon_store::update_settings(|s| s.remote_enabled = on) {
        eprintln!("[xg] save remote_enabled: {error}");
    }
}

pub(crate) fn effective_port() -> u16 {
    let saved = xenon_store::load_settings().unwrap_or_default().remote_port;
    xenon_remote::effective_port(saved, xenon_store::DEFAULT_REMOTE_PORT)
}

impl App {
    /// Start at launch when it was left on last time.
    pub(crate) fn remote_boot(self: &Rc<Self>) {
        if xenon_store::load_settings()
            .unwrap_or_default()
            .remote_enabled
        {
            self.remote_start();
        }
        self.remote_publish();
    }

    /// Menu, palette or Settings switch. Remembers the choice for next launch.
    pub(crate) fn remote_toggle(self: &Rc<Self>) {
        if self.remote.running() || self.remote.waiting() {
            self.remote_stop();
        } else {
            self.remote_start();
        }
        let on = self.remote.running() || self.remote.waiting();
        persist_enabled(on);
        let text = match (self.remote.running(), on) {
            (true, _) => "Phone remote on",
            (false, true) => "Phone remote on \u{2014} the other Xenon has it until it quits",
            (false, false) => "Phone remote off",
        };
        self.toast(text);
    }

    pub(crate) fn remote_start(self: &Rc<Self>) {
        if self.remote.running() {
            return;
        }
        let reach = Reachability::probe_local();
        let (tx, rx) = async_channel::unbounded::<HostRequest>();
        let port = effective_port();
        let server = match RemoteServer::start(&reach.bind_ips(), port, tx) {
            Ok(server) => server,
            Err(StartError::PortBusy) => {
                self.remote_wait_for_port();
                self.remote_publish();
                return;
            }
            Err(error) => {
                eprintln!("[xg] phone remote failed to start: {error:#}");
                self.toast(&format!("Phone remote didn\u{2019}t start: {error}"));
                return;
            }
        };
        server.set_uploads_dir(xenon_store::data_dir().join("remote").join("uploads"));
        eprintln!("[xg] phone remote on: {:?}", server.binds);
        self.remote.wait_logged.set(false);
        let weak = Rc::downgrade(self);
        let net_timer = glib::timeout_add_local(NETWORK_EVERY, {
            let weak = weak.clone();
            move || {
                if let Some(app) = weak.upgrade() {
                    app.remote_reprobe();
                }
                glib::ControlFlow::Continue
            }
        });
        *self.remote.rt.borrow_mut() = Some(Runtime {
            port: server.port,
            _server: server,
            reach,
            host_name: String::new(),
            devices: DeviceBook::load(),
            conns: BTreeMap::new(),
            keep_awake: Inhibitor::default(),
            net_timer: Some(net_timer),
        });
        glib::spawn_future_local(async move {
            while let Ok(req) = rx.recv().await {
                let Some(app) = weak.upgrade() else { break };
                app.remote_request(req);
            }
        });
        self.remote_names();
        self.remote_publish();
    }

    pub(crate) fn remote_stop(self: &Rc<Self>) {
        if let Some(source) = self.remote.wait.borrow_mut().take() {
            source.remove();
        }
        let runtime = self.remote.rt.borrow_mut().take();
        if let Some(mut runtime) = runtime {
            let attached: Vec<_> = runtime
                .conns
                .values()
                .filter_map(|c| c.attach.as_ref().map(|a| (a.ws, a.tab)))
                .collect();
            if let Some(source) = runtime.net_timer.take() {
                source.remove();
            }
            drop(runtime);
            self.remote.fits.borrow_mut().clear();
            for (ws, tab) in attached {
                self.fit_apply(ws, tab);
            }
            eprintln!("[xg] phone remote off");
        }
        self.remote_publish();
    }

    /// The other Xenon holds the port: look again in a few seconds. Quiet, and
    /// never bind a different port: paired phones know this one.
    fn remote_wait_for_port(self: &Rc<Self>) {
        if self.remote.waiting() {
            return;
        }
        if !self.remote.wait_logged.replace(true) {
            eprintln!("[xg] phone remote: port busy, waiting for the other Xenon");
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(PORT_RETRY, move || {
            let Some(app) = weak.upgrade() else { return };
            *app.remote.wait.borrow_mut() = None;
            app.remote_start();
        });
        *self.remote.wait.borrow_mut() = Some(id);
    }

    /// Tailscale came up or changed address: rebind so the phone can connect.
    fn remote_reprobe(self: &Rc<Self>) {
        let changed =
            self.remote.rt.borrow().as_ref().is_some_and(|rt| {
                Reachability::probe_local().tailscale_ip != rt.reach.tailscale_ip
            });
        if changed {
            eprintln!("[xg] phone remote: network changed, rebinding");
            self.remote_stop();
            self.remote_start();
        }
    }

    /// MagicDNS and computer names come from subprocesses: off the UI thread.
    fn remote_names(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let names = gio::spawn_blocking(|| {
                (
                    xenon_remote::magic_dns_name(),
                    xenon_remote::computer_name(),
                )
            })
            .await;
            let (Ok((dns, name)), Some(app)) = (names, weak.upgrade()) else {
                return;
            };
            if let Some(rt) = app.remote.rt.borrow_mut().as_mut() {
                rt.reach.magic_dns = dns;
                rt.host_name = name;
            }
            app.remote_publish();
        });
    }

    pub(crate) fn remote_status(&self) -> RemoteStatus {
        let settings = xenon_store::load_settings().unwrap_or_default();
        let hostname = xenon_remote::normalize_hostname(&settings.remote_hostname);
        let rt = self.remote.rt.borrow();
        let mut status = RemoteStatus {
            enabled: rt.is_some(),
            waiting: self.remote.waiting(),
            keep_awake: settings.remote_keep_awake_while_connected,
            hostname: hostname.clone(),
            ..Default::default()
        };
        let devices = match rt.as_ref() {
            Some(rt) => {
                let host = rt.reach.phone_host(&hostname);
                status.base_url = Some(xenon_remote::base_url(&host, rt.port));
                status.reachable = rt.reach.reachable();
                status.connected = rt.conns.len();
                rt.devices.devices().to_vec()
            }
            None => {
                xenon_store::load_remote_devices()
                    .unwrap_or_default()
                    .devices
            }
        };
        status.devices = xenon_remote::device_rows(&devices, |id| {
            rt.as_ref()
                .is_some_and(|rt| rt.conns.values().any(|c| c.device_id == id))
        });
        status
    }

    /// Recompute the status and tell everything that shows it.
    pub(crate) fn remote_publish(self: &Rc<Self>) {
        let status = self.remote_status();
        let hold = xenon_remote::should_hold(status.enabled, status.keep_awake, status.connected);
        if let Some(rt) = self.remote.rt.borrow_mut().as_mut() {
            rt.keep_awake.sync(hold);
        }
        self.refresh_connect_dialog();
        let callback = self.remote.on_change.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}
