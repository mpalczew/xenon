use std::time::Instant;

use xenon_remote::{ClosedReason, ConnId, DeviceBook, RemoteStatus, ServerMsg, device_rows};

use super::{Phone, locked};
use crate::api::Pairing;

impl Phone {
    pub(crate) fn status(&self) -> RemoteStatus {
        let settings = xenon_store::load_service_settings().unwrap_or_default();
        let hostname = xenon_remote::normalize_hostname(&settings.remote_hostname);
        let phones: Vec<String> = locked(&self.conns)
            .values()
            .map(|c| c.device_id.clone())
            .collect();
        let server = locked(&self.server);
        let mut status = RemoteStatus::default();
        status.enabled = server.rt.is_some();
        status.problem = server.problem.clone();
        status.keep_awake = settings.remote_keep_awake_while_connected;
        status.hostname = hostname.clone();
        let devices = match server.rt.as_ref() {
            Some(rt) => {
                let host = rt.reach.phone_host(&hostname);
                status.base_url = Some(xenon_remote::base_url(&host, rt.port));
                status.reachable = rt.reach.reachable();
                status.connected = phones.len();
                rt.devices.devices().to_vec()
            }
            None => {
                xenon_store::load_remote_devices()
                    .unwrap_or_default()
                    .devices
            }
        };
        status.devices = device_rows(&devices, |id| phones.iter().any(|p| p == id));
        status
    }

    pub(crate) fn pairing(&self) -> Result<Pairing, String> {
        let hostname = xenon_remote::normalize_hostname(
            &xenon_store::load_service_settings()
                .unwrap_or_default()
                .remote_hostname,
        );
        let mut guard = locked(&self.server);
        let problem = guard.problem.clone();
        let rt = guard.rt.as_mut().ok_or_else(|| {
            problem.unwrap_or_else(|| "the phone remote is not running".to_string())
        })?;
        let code = rt.devices.pairing_code(Instant::now()).clone();
        let base = xenon_remote::base_url(&rt.reach.phone_host(&hostname), rt.port);
        let reachable = rt.reach.reachable() || !hostname.is_empty();
        Ok(Pairing {
            digits: code.display_digits(),
            url: reachable.then(|| xenon_remote::pairing_url(&base, &code.secret)),
            expires_in_secs: code
                .expires_at
                .saturating_duration_since(Instant::now())
                .as_secs(),
        })
    }

    pub(crate) fn revoke(&self, device: Option<&str>) {
        {
            let mut guard = locked(&self.server);
            let mut owned;
            let book = match guard.rt.as_mut() {
                Some(rt) => &mut rt.devices,
                None => {
                    owned = DeviceBook::load();
                    &mut owned
                }
            };
            book.reload();
            match device {
                Some(id) => {
                    book.revoke(id);
                }
                None => book.revoke_all(),
            }
            if let Err(error) = book.save() {
                log::warn!("remote devices save: {error}");
            }
        }
        let doomed: Vec<ConnId> = locked(&self.conns)
            .iter()
            .filter(|(_, c)| device.is_none_or(|d| c.device_id == d))
            .map(|(id, _)| *id)
            .collect();
        for conn in doomed {
            self.send_to(
                conn,
                ServerMsg::Closed {
                    reason: ClosedReason::Revoked,
                },
            );
            self.disconnected(conn);
        }
        self.changed();
    }
}
