//! Visual-test fixtures: a running remote with fixed address, code, and
//! devices so captures are identical on every machine.

use std::net::{IpAddr, Ipv4Addr};

use xenon_remote::{Outbox, PAIRING_TTL, PairingCode};
use xenon_store::{RemoteDevice, RemoteDevices};

use super::devices::unix_now;
use super::session::Conn;
use super::*;

const HOUR: u64 = 60 * 60;

impl XenonApp {
    /// Remote "on" with a fixed tailnet address (or none) and two devices,
    /// the first connected.
    pub(crate) fn visual_remote(&mut self, reachable: bool, cx: &mut Context<Self>) {
        self.stop_mobile_remote(cx);
        let (tx, rx) = async_channel::unbounded::<HostRequest>();
        let server = RemoteServer::start(&[IpAddr::V4(Ipv4Addr::LOCALHOST)], 0, tx)
            .expect("visual remote server");
        let reach = if reachable {
            Reachability {
                tailscale_ip: Some(Ipv4Addr::new(100, 101, 2, 3)),
                magic_dns: Some("mbp.tail7c2e.ts.net".into()),
                lan_ip: None,
            }
        } else {
            Reachability::default()
        };
        let mut devices = DeviceBook::from_devices(fixture_devices());
        devices.visual_set_pairing(PairingCode {
            digits: "482913".into(),
            secret: "9f2c4e8a1b7d4c3e8f6a2b1c9d0e7f41".into(),
            expires_at: Instant::now() + PAIRING_TTL,
        });
        let (out, _) = Outbox::channel();
        let mut conns = BTreeMap::new();
        conns.insert(ConnId(1), Conn::visual("iphone-home", out));
        self.services.remote = Some(RemoteRuntime {
            _server: server,
            port: xenon_store::DEFAULT_REMOTE_PORT,
            network: RemoteNetwork::Tailscale,
            reach,
            host_name: "MacBook Pro".into(),
            devices,
            conns,
            _keep_awake: None,
            _requests: self.spawn_remote_requests(rx, cx),
            _tick: Task::ready(()),
            ticks: 0,
        });
        self.publish_remote_info(cx);
    }

    /// Remote on, but no phone connected yet (sidebar shows "listening").
    pub(crate) fn visual_remote_listening(&mut self, reachable: bool, cx: &mut Context<Self>) {
        self.visual_remote(reachable, cx);
        if let Some(runtime) = self.services.remote.as_mut() {
            runtime.conns.clear();
        }
        self.publish_remote_info(cx);
    }
}

fn fixture_devices() -> RemoteDevices {
    let now = unix_now();
    let device = |id: &str, label: &str, seen_hours: u64| RemoteDevice {
        id: id.into(),
        label: label.into(),
        token_sha256: String::new(),
        created_at: now - 3 * 24 * HOUR,
        last_seen_at: now - seen_hours * HOUR,
    };
    RemoteDevices {
        devices: vec![
            device("iphone-home", "iPhone · Home Screen", 0),
            device("iphone-safari", "iPhone · Safari", 2),
        ],
    }
}
