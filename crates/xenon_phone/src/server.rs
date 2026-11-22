use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::remote::{DeviceBook, HostRequest, Reachability, RemoteServer, StartError};

use super::{Phone, Runtime, locked};

const UPKEEP: Duration = Duration::from_secs(5);
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
                thread::sleep(UPKEEP);
                let Some(phone) = weak.upgrade() else { break };
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
        let started = RemoteServer::start(
            &reach.bind_ips(),
            crate::remote::effective_port(),
            self.tx.clone(),
        );
        let server = match started {
            Ok(server) => server,
            Err(error) => {
                log::error!("phone remote failed to start: {error:#}");
                let text = match error {
                    StartError::PortBusy => {
                        format!("port {} is busy", crate::remote::effective_port())
                    }
                    other => format!("{other:#}"),
                };
                locked(&self.server).problem = Some(text);
                self.changed();
                return;
            }
        };
        server.set_uploads_dir(xenon_store::data_dir().join("remote").join("uploads"));
        log::info!("phone remote on: {:?}", server.binds);
        {
            let mut guard = locked(&self.server);
            guard.problem = None;
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

    fn look_up_dns_name(&self) {
        let weak = self.self_ref();
        let spawned = thread::Builder::new()
            .name("xenon-phone-dns".into())
            .spawn(move || {
                let dns = crate::remote::magic_dns_name();
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
            guard.problem = None;
            guard.rt.take()
        };
        if let Some(runtime) = runtime {
            drop(runtime);
            log::info!("phone remote off");
        }
        self.changed();
    }

    pub(crate) fn apply_settings(&self) {
        let on = xenon_store::load_service_settings()
            .unwrap_or_default()
            .remote_enabled;
        let running = {
            let guard = locked(&self.server);
            guard.rt.is_some() || guard.problem.is_some()
        };
        match (on, running) {
            (true, false) => self.start_server(),
            (false, true) => self.stop_server(),
            _ => {}
        }
    }

    pub(crate) fn watch_settings(&self) {
        let path = xenon_store::service_settings_path();
        let Some(dir) = path.parent() else { return };
        let weak = self.self_ref();
        let started = xenon_term::watch_dir(dir, move || {
            if let Some(phone) = weak.upgrade() {
                phone.apply_settings();
            }
        });
        if let Err(error) = started {
            log::warn!("watch {}: {error}", dir.display());
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
