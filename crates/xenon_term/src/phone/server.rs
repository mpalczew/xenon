//! Starting and stopping the HTTP server, and keeping it right as the network changes.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use xenon_remote::{DeviceBook, HostRequest, Reachability, RemoteServer, StartError};

use super::{Phone, Runtime, locked};

const PORT_RETRY: Duration = Duration::from_secs(5);
/// How many retry ticks make one network re-probe (30 seconds).
const REPROBE_TICKS: u32 = 6;

pub(super) fn spawn_threads(phone: &Arc<Phone>, rx: async_channel::Receiver<HostRequest>) {
    let weak = Arc::downgrade(phone);
    let requests = thread::Builder::new()
        .name("xenon-phone-requests".into())
        .spawn(move || {
            while let Ok(req) = rx.recv_blocking() {
                let Some(phone) = weak.upgrade() else { break };
                phone.request(req);
            }
        });
    let weak = Arc::downgrade(phone);
    let upkeep = thread::Builder::new()
        .name("xenon-phone-upkeep".into())
        .spawn(move || {
            let mut ticks = 0;
            loop {
                thread::sleep(PORT_RETRY);
                let Some(phone) = weak.upgrade() else { break };
                if locked(&phone.server).waiting {
                    phone.start_server();
                }
                ticks += 1;
                if ticks % REPROBE_TICKS == 0 {
                    phone.reprobe();
                }
            }
        });
    for spawned in [requests, upkeep] {
        if let Err(error) = spawned {
            log::error!("phone thread: {error}");
        }
    }
}

pub(crate) fn effective_port() -> u16 {
    let saved = xenon_store::load_settings().unwrap_or_default().remote_port;
    xenon_remote::effective_port(saved, xenon_store::DEFAULT_REMOTE_PORT)
}

fn host_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

impl Phone {
    pub(crate) fn start_server(&self) {
        if locked(&self.server).rt.is_some() {
            return;
        }
        let reach = Reachability::probe_local();
        let started = RemoteServer::start(&reach.bind_ips(), effective_port(), self.tx.clone());
        let server = match started {
            Ok(server) => server,
            Err(StartError::PortBusy) => {
                let first = !std::mem::replace(&mut locked(&self.server).waiting, true);
                if first {
                    log::info!("phone remote: port busy, waiting for the other Xenon");
                    self.changed();
                }
                return;
            }
            Err(error) => {
                log::error!("phone remote failed to start: {error:#}");
                return;
            }
        };
        server.set_uploads_dir(xenon_store::data_dir().join("remote").join("uploads"));
        log::info!("phone remote on: {:?}", server.binds);
        {
            let mut guard = locked(&self.server);
            guard.waiting = false;
            guard.rt = Some(Runtime {
                port: server.port,
                _server: server,
                reach,
                host_name: host_name(),
                devices: DeviceBook::load(),
            });
        }
        self.look_up_dns_name();
        self.changed();
    }

    /// Tailscale's MagicDNS name takes a subprocess to find: do it off the caller's thread.
    fn look_up_dns_name(&self) {
        let weak = self.self_ref();
        let spawned = thread::Builder::new()
            .name("xenon-phone-dns".into())
            .spawn(move || {
                let dns = xenon_remote::magic_dns_name();
                let Some(phone) = weak.upgrade() else { return };
                if let Some(rt) = locked(&phone.server).rt.as_mut() {
                    rt.reach.magic_dns = dns;
                }
                phone.changed();
            });
        if let Err(error) = spawned {
            log::warn!("phone dns thread: {error}");
        }
    }

    pub(crate) fn stop_server(&self) {
        let runtime = {
            let mut guard = locked(&self.server);
            guard.waiting = false;
            guard.rt.take()
        };
        if let Some(runtime) = runtime {
            // Dropping joins the accept threads; phones are told the server is stopping.
            drop(runtime);
            log::info!("phone remote off");
        }
        self.changed();
    }

    /// Switch the server on or off and remember the choice.
    pub(crate) fn set_enabled(&self, on: bool) {
        if let Err(error) = xenon_store::update_settings(|s| s.remote_enabled = on) {
            log::warn!("save remote_enabled: {error}");
        }
        let running = {
            let guard = locked(&self.server);
            guard.rt.is_some() || guard.waiting
        };
        match (on, running) {
            (true, false) => self.start_server(),
            (false, true) => self.stop_server(),
            _ => {}
        }
    }

    fn reprobe(&self) {
        let changed = locked(&self.server)
            .rt
            .as_ref()
            .is_some_and(|rt| Reachability::probe_local().tailscale_ip != rt.reach.tailscale_ip);
        if changed {
            log::info!("phone remote: network changed, rebinding");
            self.stop_server();
            self.start_server();
        }
    }
}
